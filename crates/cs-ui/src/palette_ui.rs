//! Ventana Palette Presets, menú … del panel de paleta y sus diálogos.

use cs_core::Palette;
#[cfg(not(target_arch = "wasm32"))]
use cs_io::lospec::LospecSort;
use cs_io::lospec::{LospecPage, LospecPalette};
use cs_io::palette_files::{self, NamedPalette, PaletteFormat};
use eframe::egui;

use crate::ChimpApp;
use crate::lospec_client::{self, FetchError, Pending};
#[cfg(not(target_arch = "wasm32"))]
use crate::lospec_examples::{self, ExampleCache};
#[cfg(not(target_arch = "wasm32"))]
use crate::lospec_tab::CountKind;
use crate::lospec_tab::LospecTab;
use crate::palette_library::{Preset, PresetSource};
use crate::panels::{self, PaletteAction};
use crate::pixel_ui;
use crate::pixel_ui::Tip as _;
use crate::platform::{self, FileFilter, OpenPurpose};

const LOAD_FILTERS: &[FileFilter] = &[FileFilter {
    name: "Palettes",
    extensions: &["gpl", "hex", "pal", "png", "ase", "aseprite"],
}];

const SAVE_FILTERS: &[FileFilter] = &[
    FileFilter {
        name: "GIMP Palette",
        extensions: &["gpl"],
    },
    FileFilter {
        name: "Hex",
        extensions: &["hex"],
    },
    FileFilter {
        name: "JASC Palette",
        extensions: &["pal"],
    },
];

/// Colores visibles por fila en la ventana.
const STRIP_COLORS: usize = 64;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PresetsTab {
    #[default]
    Library,
    Lospec,
}

#[derive(Default)]
pub struct PresetsWindow {
    pub tab: PresetsTab,
    search: String,
    lospec: LospecTab,
    requests: LospecRequests,
    /// El campo del link de una paleta (el botón de la cadena).
    link_open: bool,
    /// Imágenes de ejemplo de Lospec; se sueltan al cerrar la ventana.
    #[cfg(not(target_arch = "wasm32"))]
    examples: ExampleCache,
}

/// Plazo de una petición a Lospec. `ehttp` no corta una conexión colgada antes de recibir los
/// encabezados, así que el corte se hace acá.
pub(crate) const REQUEST_TIMEOUT_SECS: f64 = 30.0;

/// Una petición en curso: la `generation` con la que se pidió y cuándo empezó (`i.time` de egui,
/// en segundos; sirve también en web, a diferencia de `Instant`).
struct InFlight<T> {
    generation: u64,
    started: f64,
    pending: Pending<T>,
}

impl<T> InFlight<T> {
    fn new(generation: u64, started: f64, pending: Pending<T>) -> Self {
        Self {
            generation,
            started,
            pending,
        }
    }
}

/// La respuesta de `slot` si llegó, o `Network` si venció el plazo; en los dos casos se suelta.
fn take_response<T>(
    slot: &mut Option<InFlight<T>>,
    now: f64,
) -> Option<(u64, Result<T, FetchError>)> {
    let request = slot.as_ref()?;
    let result = request.pending.poll().or_else(|| {
        (now - request.started > REQUEST_TIMEOUT_SECS).then_some(Err(FetchError::Network))
    })?;
    let generation = request.generation;
    *slot = None;
    Some((generation, result))
}

/// Peticiones en curso.
#[derive(Default)]
struct LospecRequests {
    listing: Option<InFlight<LospecPage>>,
    exact: Option<InFlight<LospecPalette>>,
    lookup: Option<InFlight<LospecPalette>>,
}

impl LospecRequests {
    /// Entrega a la pestaña lo que haya llegado y corta lo que pasó el plazo (`now`: `i.time`).
    /// Si una respuesta llega después del corte, ya no hay quién la entregue.
    fn poll(&mut self, tab: &mut LospecTab, now: f64) {
        if let Some((g, r)) = take_response(&mut self.listing, now) {
            tab.finish_listing(g, r);
        }
        if let Some((g, r)) = take_response(&mut self.exact, now) {
            tab.finish_exact(g, r);
        }
        if let Some((g, r)) = take_response(&mut self.lookup, now) {
            tab.finish_lookup(g, r);
        }
    }

    fn is_busy(&self) -> bool {
        self.listing.is_some() || self.exact.is_some() || self.lookup.is_some()
    }
}

/// Lo que se guarda en la biblioteca para un resultado de Lospec.
fn lospec_preset(p: &LospecPalette) -> (NamedPalette, Vec<String>) {
    (p.named(), p.comments())
}

/// Clic en un botón de una fila de Lospec.
enum LospecAction {
    Apply(LospecPalette),
    Save(LospecPalette),
}

/// Pedir el nombre para "Save as Preset" (y para "Set as Default" si la paleta no es un preset).
pub struct NameDialog {
    name: String,
    /// La paleta al abrir el diálogo: es la que se guarda aunque luego cambie la del sprite.
    palette: Palette,
    then_default: bool,
    /// El foco se pide solo en el primer cuadro (después el usuario puede moverlo).
    focused: bool,
}

/// Confirmaciones pendientes.
pub enum Confirm {
    Replace {
        palette: NamedPalette,
        comments: Vec<String>,
        then_default: bool,
    },
    Delete(String),
    /// "New palette" en un sprite indexado: los píxeles de los colores que se van cambian.
    NewPalette,
}

/// Formato al guardar, según la extensión elegida (`.gpl` si no se reconoce).
fn save_format(file_name: &str) -> PaletteFormat {
    match palette_files::format_for(file_name) {
        Some(f @ (PaletteFormat::Hex | PaletteFormat::Pal)) => f,
        _ => PaletteFormat::Gpl,
    }
}

/// Nombre final del archivo: si la extensión no es `.gpl`, `.hex` ni `.pal` (p. ej. `.png`),
/// pasa a `.gpl` para no escribir texto GPL dentro de un `.png`.
fn save_file_name(file_name: &str, stem: &str) -> String {
    match palette_files::format_for(file_name) {
        Some(PaletteFormat::Gpl | PaletteFormat::Hex | PaletteFormat::Pal) => file_name.to_string(),
        _ => format!("{stem}.gpl"),
    }
}

fn matches_search(name: &str, search: &str) -> bool {
    name.to_lowercase().contains(&search.trim().to_lowercase())
}

impl ChimpApp {
    pub(crate) fn palette_action(&mut self, ctx: &egui::Context, action: PaletteAction) {
        match action {
            PaletteAction::OpenPresets => {
                self.palettes.reload();
                self.presets_window = Some(PresetsWindow::default());
            }
            PaletteAction::Load => {
                self.platform
                    .open_file(LOAD_FILTERS, OpenPurpose::LoadPalette, ctx)
            }
            PaletteAction::Save => self.save_palette_file(),
            PaletteAction::SaveAsPreset => self.open_name_dialog(false),
            PaletteAction::SetAsDefault => self.set_current_as_default(),
            // Bloqueada no se reemplaza; en indexado se pregunta antes (`Confirm::NewPalette`).
            PaletteAction::NewPalette if self.doc.editor.palette_locked => {}
            PaletteAction::NewPalette if self.doc.editor.needs_new_palette_confirmation() => {
                self.palette_confirm = Some(Confirm::NewPalette);
            }
            PaletteAction::NewPalette => {
                self.doc.editor.new_palette();
            }
            PaletteAction::LoadDefault => {
                let palette = self.palettes.default_palette();
                self.doc.editor.apply_palette(palette);
            }
            PaletteAction::OpenFolder => {
                if let Some(dir) = self.palettes.folder() {
                    let _ = std::fs::create_dir_all(&dir);
                    if let Err(e) = platform::open_folder(&dir) {
                        self.show_message("Could not open the folder", e);
                    }
                }
            }
        }
    }

    pub(crate) fn load_palette_bytes(&mut self, name: &str, bytes: &[u8]) {
        match palette_files::read_palette(name, bytes) {
            Ok(p) => {
                self.doc.editor.apply_palette(p.palette);
            }
            Err(e) => self.show_message("Could not load the palette", e.to_string()),
        }
    }

    fn current_palette(&self, name: String) -> NamedPalette {
        NamedPalette {
            name,
            palette: self.doc.editor.sprite().palette.clone(),
        }
    }

    fn open_name_dialog(&mut self, then_default: bool) {
        self.preset_name = Some(NameDialog {
            name: self.doc.tab_name_stem(),
            palette: self.doc.editor.sprite().palette.clone(),
            then_default,
            focused: false,
        });
    }

    fn save_palette_file(&mut self) {
        let suggested = format!("{}.gpl", self.doc.tab_name_stem());
        let Some(mut target) = self.platform.choose_save(&suggested, SAVE_FILTERS) else {
            return;
        };
        target.file_name = save_file_name(&target.file_name, target.stem());
        let palette = self.current_palette(target.stem().to_string());
        let bytes = palette_files::write_palette(save_format(&target.file_name), &palette)
            .expect("Gpl, Hex y Pal se pueden escribir");
        if let Err(e) = self.platform.write(&target, &bytes) {
            self.show_message("Could not save the palette", e);
        }
    }

    /// Si la paleta actual ya es un preset, lo marca (primero los del usuario, con la
    /// carpeta recién leída); si no, primero pide un nombre.
    fn set_current_as_default(&mut self) {
        self.palettes.reload();
        let current = &self.doc.editor.sprite().palette;
        let existing = self
            .palettes
            .user()
            .iter()
            .chain(self.palettes.builtin())
            .find(|p| p.palette == *current)
            .map(|p| (p.source, p.name.clone()));
        match existing {
            Some((source, name)) => self.palettes.set_default(source, &name),
            None => self.open_name_dialog(true),
        }
    }

    /// Guarda en la biblioteca y, si se pidió, la marca como la paleta por defecto.
    pub(crate) fn store_preset(
        &mut self,
        palette: NamedPalette,
        comments: Vec<String>,
        then_default: bool,
    ) {
        let name = palette.name.clone();
        match self.palettes.save(&palette, &comments) {
            Ok(()) if then_default => self.palettes.set_default(PresetSource::User, &name),
            Ok(()) => {}
            Err(e) => self.show_message("Could not save the preset", e),
        }
    }

    /// Pregunta si reemplazar cuando ya existe un preset de usuario con ese nombre.
    pub(crate) fn save_preset_asking(
        &mut self,
        palette: NamedPalette,
        comments: Vec<String>,
        then_default: bool,
    ) {
        if self.palettes.has_user(&palette.name) {
            self.palette_confirm = Some(Confirm::Replace {
                palette,
                comments,
                then_default,
            });
        } else {
            self.store_preset(palette, comments, then_default);
        }
    }

    /// La ventana de presets está abierta en la pestaña Lospec (el botón del dock).
    #[cfg(test)]
    pub(crate) fn lospec_open(&self) -> bool {
        self.lospec_dock.is_some()
    }

    /// El panel Lospec del dock, flotando junto a su botón (`anchor`): lo mismo que la pestaña
    /// Lospec de Palette Presets. Su estado vive mientras está abierto (ver `close_lospec_panel`).
    pub(crate) fn lospec_panel(&mut self, ctx: &egui::Context, anchor: egui::Rect, modal: bool) {
        let mut window = match self.lospec_dock.take() {
            Some(window) => window,
            None => {
                self.palettes.reload();
                #[allow(unused_mut)]
                let mut window = PresetsWindow {
                    tab: PresetsTab::Lospec,
                    ..Default::default()
                };
                // Para empezar con algo: las paletas más nuevas.
                #[cfg(not(target_arch = "wasm32"))]
                {
                    window.lospec.sort = LospecSort::Newest;
                    start_search(ctx, &mut window.lospec, &mut window.requests);
                }
                window
            }
        };
        poll_lospec(ctx, &mut window);
        let mut action = None;
        let library = &self.palettes;
        let skin = self.pixel.skin();
        crate::dock::flyout(
            ctx,
            crate::dock::DockPanel::Lospec,
            anchor,
            skin,
            LOSPEC_PANEL_W,
            |ui| {
                lospec_band(ui, skin);
                ui.add_enabled_ui(!modal, |ui| {
                    // La lista hasta abajo de la pantalla.
                    let bottom = ctx.content_rect().bottom() - LOSPEC_BELOW;
                    action =
                        lospec_ui(ui, &mut window, |name| library.has_user(name), Some(bottom));
                });
            },
        );
        self.lospec_action(action);
        self.lospec_dock = Some(window);
    }

    /// Cerrado el panel Lospec, se sueltan sus imágenes y peticiones.
    pub(crate) fn close_lospec_panel(&mut self) {
        self.lospec_dock = None;
    }

    fn lospec_action(&mut self, action: Option<LospecAction>) {
        match action {
            Some(LospecAction::Apply(p)) => {
                self.doc.editor.apply_palette(p.palette.clone());
                if !self.palettes.has_user(&p.title) {
                    let (named, comments) = lospec_preset(&p);
                    self.store_preset(named, comments, false);
                }
            }
            Some(LospecAction::Save(p)) => {
                let (named, comments) = lospec_preset(&p);
                self.save_preset_asking(named, comments, false);
            }
            None => {}
        }
    }

    /// `modal`: hay un diálogo abierto (este cuadro); la ventana de presets queda desactivada.
    pub(crate) fn update_palette_windows(&mut self, ctx: &egui::Context, modal: bool) {
        if !self.has_doc() {
            self.presets_window = None;
            self.preset_name = None;
            self.palette_confirm = None;
            return;
        }
        self.update_presets_window(ctx, modal);
        self.update_name_dialog(ctx);
        self.update_palette_confirm(ctx);
    }

    fn update_presets_window(&mut self, ctx: &egui::Context, modal: bool) {
        let Some(mut window) = self.presets_window.take() else {
            return;
        };
        let mut open = true;
        let mut apply = None;
        let mut delete = None;
        let mut set_default = None;
        // Se revisa aunque la pestaña Lospec no esté a la vista, para no perder respuestas.
        poll_lospec(ctx, &mut window);
        let mut lospec_action = None;
        let skin = self.pixel.skin();
        let (_, closed) = pixel_ui::app_window(
            ctx,
            skin,
            "palette_presets_window",
            "Palette Presets",
            true,
            (egui::Align2::CENTER_CENTER, egui::Vec2::ZERO),
            |ui| {
                // Tamaño fijo (la ventana no se estira): la lista scrollea adentro.
                ui.set_width(PRESETS_SIZE.x);
                ui.set_height(PRESETS_SIZE.y);
                ui.add_enabled_ui(!modal, |ui| {
                    // Con Pixel UI, las pestañas del diseño (como las de Artboards).
                    const TABS: [(PresetsTab, &str); 2] = [
                        (PresetsTab::Library, "Library"),
                        (PresetsTab::Lospec, "Lospec"),
                    ];
                    match skin {
                        Some(skin) => {
                            let tabs: Vec<_> = TABS
                                .iter()
                                .map(|(_, name)| crate::pixel_sections::Tab {
                                    name: (*name).to_owned(),
                                    color: None,
                                })
                                .collect();
                            let active = TABS.iter().position(|(t, _)| *t == window.tab);
                            if let Some(i) = crate::pixel_sections::tabs(ui, skin, &tabs, active) {
                                window.tab = TABS[i].0;
                            }
                        }
                        None => {
                            ui.horizontal(|ui| {
                                for (tab, name) in TABS {
                                    ui.selectable_value(&mut window.tab, tab, name);
                                }
                            });
                            ui.separator();
                        }
                    }
                    match window.tab {
                        PresetsTab::Library => {
                            ui.horizontal(|ui| {
                                ui.label("Search");
                                crate::widgets::text_line(ui, &mut window.search);
                            });
                            crate::pixel_ui::scroll(ui, egui::ScrollArea::vertical(), |ui| {
                                for (title, list) in [
                                    ("Built-in", self.palettes.builtin()),
                                    ("User", self.palettes.user()),
                                ] {
                                    ui.strong(title);
                                    let shown: Vec<&Preset> = list
                                        .iter()
                                        .filter(|p| matches_search(&p.name, &window.search))
                                        .collect();
                                    if shown.is_empty() {
                                        ui.weak("No palettes");
                                    }
                                    for preset in shown {
                                        let is_default = self.palettes.is_default(preset);
                                        let response = preset_row(ui, preset, is_default);
                                        if response.clicked() {
                                            apply = Some(preset.palette.clone());
                                        }
                                        response.context_menu(|ui| {
                                            if ui.button("Set as Default").clicked() {
                                                set_default =
                                                    Some((preset.source, preset.name.clone()));
                                                ui.close();
                                            }
                                            if preset.deletable && ui.button("Delete").clicked() {
                                                delete = Some(preset.name.clone());
                                                ui.close();
                                            }
                                        });
                                    }
                                    ui.add_space(8.0);
                                }
                            });
                        }
                        PresetsTab::Lospec => {
                            let library = &self.palettes;
                            lospec_action =
                                lospec_ui(ui, &mut window, |name| library.has_user(name), None);
                        }
                    }
                });
            },
        );
        open &= !closed;
        if let Some(palette) = apply {
            self.doc.editor.apply_palette(palette);
        }
        if let Some((source, name)) = set_default {
            self.palettes.set_default(source, &name);
        }
        if let Some(name) = delete {
            self.palette_confirm = Some(Confirm::Delete(name));
        }
        self.lospec_action(lospec_action);
        if open {
            self.presets_window = Some(window);
        }
    }

    fn update_name_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.preset_name.take() else {
            return;
        };
        let mut result = None;
        let skin = self.pixel.skin();
        let (_, closed) = pixel_ui::app_window(
            ctx,
            skin,
            "save_preset_window",
            "Save as Preset",
            true,
            (egui::Align2::CENTER_CENTER, egui::Vec2::ZERO),
            |ui| {
                let response = crate::widgets::text_line(ui, &mut dialog.name);
                if !dialog.focused {
                    response.request_focus();
                    dialog.focused = true;
                }
                let enter = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                let valid = !dialog.name.trim().is_empty();
                ui.add_space(6.0);
                let clicked = pixel_ui::dialog_buttons(ui, skin, &["Save", "Cancel"]);
                if (clicked == Some(0) || enter) && valid {
                    result = Some(true);
                }
                if clicked == Some(1) || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    result = Some(false);
                }
            },
        );
        if closed {
            result = Some(false);
        }
        match result {
            None => self.preset_name = Some(dialog),
            Some(false) => {}
            Some(true) => {
                let palette = NamedPalette {
                    name: dialog.name.trim().to_string(),
                    palette: dialog.palette,
                };
                self.save_preset_asking(palette, Vec::new(), dialog.then_default);
            }
        }
    }

    fn update_palette_confirm(&mut self, ctx: &egui::Context) {
        let Some(confirm) = self.palette_confirm.take() else {
            return;
        };
        let (title, body, ok) = match &confirm {
            Confirm::Replace { palette, .. } => (
                "Replace preset?",
                format!("A preset named \"{}\" already exists.", palette.name),
                "Replace",
            ),
            Confirm::Delete(name) => (
                "Delete preset?",
                format!("\"{name}\" will be removed from your presets."),
                "Delete",
            ),
            Confirm::NewPalette => ("New palette", NEW_PALETTE_QUESTION.to_owned(), "OK"),
        };
        // Con Pixel UI, en el diálogo pixel, como "Unsaved changes".
        let skin = self.pixel.skin();
        let mut result = confirm_window(ctx, skin, title, &body, ok);
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            result = Some(false);
        }
        match (result, confirm) {
            (None, confirm) => self.palette_confirm = Some(confirm),
            (Some(false), _) => {}
            (
                Some(true),
                Confirm::Replace {
                    palette,
                    comments,
                    then_default,
                },
            ) => self.store_preset(palette, comments, then_default),
            (Some(true), Confirm::NewPalette) => {
                self.doc.editor.new_palette();
            }
            (Some(true), Confirm::Delete(name)) => {
                if let Err(e) = self.palettes.delete(&name) {
                    self.show_message("Could not delete the preset", e);
                }
            }
        }
    }
}

/// La pregunta de "New palette" en un sprite indexado.
const NEW_PALETTE_QUESTION: &str = "Replace the palette? Pixels using removed colors will change.";

/// Una confirmación en la ventana de la app, como "Unsaved changes": la pregunta, `ok` y Cancel.
/// Cerrar con la × es Cancel.
fn confirm_window(
    ctx: &egui::Context,
    skin: Option<&crate::skin::Skin>,
    title: &str,
    body: &str,
    ok: &str,
) -> Option<bool> {
    let mut result = None;
    let (_, closed) = pixel_ui::app_window(
        ctx,
        skin,
        "palette_confirm",
        title,
        true,
        (egui::Align2::CENTER_CENTER, egui::Vec2::ZERO),
        |ui| {
            ui.vertical_centered(|ui| ui.label(body));
            ui.add_space(6.0);
            result = pixel_ui::dialog_buttons(ui, skin, &[ok, "Cancel"]).map(|i| i == 0);
        },
    );
    if closed {
        return Some(false);
    }
    result
}

/// El tamaño del contenido de la ventana de presets.
const PRESETS_SIZE: egui::Vec2 = egui::vec2(420.0, 440.0);

/// El panel Lospec del dock: el ancho de su contenido y lo que queda debajo de la lista (el borde
/// del panel y la barra de estado).
const LOSPEC_PANEL_W: f32 = 364.0;
const LOSPEC_BELOW: f32 = 40.0;
/// La franja de arriba del panel (frame 8): el logo de Lospec y las franjas de colores (de 25 px,
/// contra la luz de la derecha), de 29 de alto, debajo de la luz de arriba.
const BAND_H: f32 = 29.0;
const BAND_STRIPE: f32 = 25.0;
const BAND_LOGO: egui::Vec2 = egui::vec2(5.0, 6.0);
const BAND_COLORS: [egui::Color32; 12] = [
    egui::Color32::from_rgb(158, 68, 145),
    egui::Color32::from_rgb(99, 54, 98),
    egui::Color32::from_rgb(144, 61, 98),
    egui::Color32::from_rgb(189, 81, 90),
    egui::Color32::from_rgb(214, 154, 78),
    egui::Color32::from_rgb(243, 208, 64),
    egui::Color32::from_rgb(255, 232, 140),
    egui::Color32::from_rgb(242, 242, 240),
    egui::Color32::from_rgb(148, 224, 146),
    egui::Color32::from_rgb(31, 153, 131),
    egui::Color32::from_rgb(34, 99, 107),
    egui::Color32::from_rgb(70, 71, 98),
];

/// Arriba del panel Lospec, de borde a borde: el logo (clic: lospec.com) y las franjas; debajo,
/// la línea grabada. Ocupa su lugar en `ui`.
fn lospec_band(ui: &mut egui::Ui, skin: Option<&crate::skin::Skin>) {
    use eframe::emath::GuiRounding as _;
    let margin = f32::from(crate::dock::FLYOUT_MARGIN);
    let content = ui.max_rect();
    let top = ui.cursor().top();
    // Dentro de la línea y la luz del panel.
    let band = egui::Rect::from_min_max(
        egui::pos2(content.left() - margin + 2.0, top - margin + 2.0),
        egui::pos2(content.right() + margin - 2.0, top - margin + 2.0 + BAND_H),
    );
    let ppp = ui.ctx().pixels_per_point();
    let painter = ui.painter().with_clip_rect(band);
    for (k, &color) in BAND_COLORS.iter().rev().enumerate() {
        let right = band.right() - k as f32 * BAND_STRIPE;
        let stripe = egui::Rect::from_min_max(
            egui::pos2(right - BAND_STRIPE, band.top()),
            egui::pos2(right, band.bottom()),
        );
        painter.rect_filled(stripe.round_to_pixels(ppp), 0.0, color);
    }
    let logo = skin.and_then(|s| s.piece("lospec.logo", crate::skin::WidgetState::Normal));
    let size = logo.map_or(egui::vec2(60.0, 18.0), |p| {
        egui::vec2(p.size[0] as f32, p.size[1] as f32)
    });
    let at = egui::Rect::from_min_size(band.min + BAND_LOGO, size);
    match logo {
        Some(piece) => painter.extend(pixel_ui::piece_shapes(piece, at, ppp)),
        None => {
            painter.text(
                at.left_center(),
                egui::Align2::LEFT_CENTER,
                "LOSPEC",
                egui::TextStyle::Body.resolve(ui.style()),
                ui.visuals().text_color(),
            );
        }
    }
    let response = ui
        .interact(at, ui.id().with("lospec_logo"), egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .tip("lospec.com");
    if response.clicked()
        && let Err(e) = platform::open_url(ui.ctx(), "https://lospec.com/palette-list")
    {
        log::warn!("could not open lospec.com: {e}");
    }
    // Lo que la franja ocupa del contenido, y la línea grabada justo debajo.
    let (_, _) = ui.allocate_exact_size(
        egui::vec2(content.width(), (band.bottom() - top).max(0.0)),
        egui::Sense::hover(),
    );
    if let Some(skin) = skin {
        let line = egui::Rangef::new(
            content.left() - margin + 1.0,
            content.right() + margin - 1.0,
        );
        ui.painter().add(pixel_ui::etched_hline(
            line,
            band.bottom(),
            skin.colors(),
            ppp,
        ));
    }
    ui.add_space(pixel_ui::ETCHED_SIZE + 2.0);
}

/// Lo que llegó de Lospec (listas e imágenes de ejemplo).
fn poll_lospec(ctx: &egui::Context, window: &mut PresetsWindow) {
    let now = ctx.input(|i| i.time);
    window.requests.poll(&mut window.lospec, now);
    #[cfg(not(target_arch = "wasm32"))]
    window.examples.poll(now, |url, image| {
        ctx.load_texture(url, image, egui::TextureOptions::NEAREST)
    });
    #[cfg(not(target_arch = "wasm32"))]
    let busy = window.requests.is_busy() || window.examples.is_busy();
    #[cfg(target_arch = "wasm32")]
    let busy = window.requests.is_busy();
    // Sin repintados el plazo no se revisaría hasta que el usuario mueva algo.
    if busy {
        ctx.request_repaint_after(std::time::Duration::from_secs(1));
    }
}

/// Fila de la ventana: nombre, cantidad de colores y tira. Toda la fila es clicable.
fn preset_row(ui: &mut egui::Ui, preset: &Preset, is_default: bool) -> egui::Response {
    let inner = ui.group(|ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.label(&preset.name);
            ui.weak(format!("{} colors", preset.palette.len()));
            if !preset.author.is_empty() {
                ui.weak(format!("· {}", preset.author));
            }
            if is_default {
                ui.weak("· default");
            }
        });
        panels::palette_strip(ui, &preset.palette, STRIP_COLORS, 8.0);
    });
    ui.interact(
        inner.response.rect,
        inner.response.id.with("row"),
        egui::Sense::click(),
    )
    .on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Contenido de la pestaña Lospec. Devuelve el botón que se pulsó en alguna fila.
fn lospec_ui(
    ui: &mut egui::Ui,
    window: &mut PresetsWindow,
    in_library: impl Fn(&str) -> bool,
    list_h: Option<f32>,
) -> Option<LospecAction> {
    let ctx = ui.ctx().clone();
    let now = ctx.input(|i| i.time);
    let PresetsWindow {
        lospec: tab,
        requests,
        link_open,
        #[cfg(not(target_arch = "wasm32"))]
        examples,
        ..
    } = window;
    let mut action = None;

    #[cfg(not(target_arch = "wasm32"))]
    search_form(ui, &ctx, tab, requests, link_open);
    #[cfg(target_arch = "wasm32")]
    {
        // En la web no hay búsqueda: el link (o el nombre) es lo único.
        *link_open = true;
    }

    // El link de una paleta (o su nombre): con el botón de la cadena.
    if *link_open {
        ui.horizontal(|ui| {
            ui.label("Paste a Lospec link or palette name");
            let response = crate::widgets::text_line(ui, &mut tab.link);
            let enter = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if tab.lookup_loading() {
                ui.spinner();
            } else if (crate::pixel_ui::text_button(ui, "Fetch").clicked() || enter)
                && let Some((g, slug)) = tab.start_lookup()
            {
                requests.lookup = Some(InFlight::new(
                    g,
                    now,
                    lospec_client::fetch_palette(&ctx, &slug),
                ));
            }
        });
    }
    #[cfg(target_arch = "wasm32")]
    ui.weak("Search is available in the desktop app");

    if let Some(error) = tab.error() {
        ui.colored_label(ui.visuals().error_fg_color, error);
    }
    // La línea grabada de lado a lado (en el panel del dock, de borde a borde).
    etched_rule(ui, list_h.is_some());

    let mut area = egui::ScrollArea::vertical();
    if let Some(h) = list_h {
        let h = (h - ui.cursor().top()).max(120.0);
        area = area
            .max_height(h)
            .min_scrolled_height(h)
            .auto_shrink([false, false]);
    }
    crate::pixel_ui::scroll(ui, area, |ui| {
        ui.set_max_width(ui.available_width() - LOSPEC_SCROLL_GAP);
        if let Some(p) = tab.lookup_result() {
            #[cfg(not(target_arch = "wasm32"))]
            lospec_row(
                ui,
                p,
                false,
                in_library(&p.title),
                &mut action,
                examples,
                now,
            );
            #[cfg(target_arch = "wasm32")]
            lospec_row(ui, p, false, in_library(&p.title), &mut action);
        }
        for p in tab.results() {
            #[cfg(not(target_arch = "wasm32"))]
            lospec_row(
                ui,
                p,
                tab.is_exact(&p.slug),
                in_library(&p.title),
                &mut action,
                examples,
                now,
            );
            #[cfg(target_arch = "wasm32")]
            lospec_row(
                ui,
                p,
                tab.is_exact(&p.slug),
                in_library(&p.title),
                &mut action,
            );
        }
        #[cfg(not(target_arch = "wasm32"))]
        if tab.listing_loading() && tab.shown_of_total().0 == 0 {
            ui.weak("Loading");
        } else if tab.can_load_more() {
            ui.vertical_centered(|ui| {
                let (shown, total) = tab.shown_of_total();
                if tab.listing_loading() {
                    ui.spinner();
                } else if crate::pixel_ui::text_button(
                    ui,
                    &format!("Load more ({shown} of {total})"),
                )
                .clicked()
                    && let Some((g, query)) = tab.start_more()
                {
                    requests.listing = Some(InFlight::new(
                        g,
                        now,
                        lospec_client::fetch_listing(&ctx, &query),
                    ));
                }
            });
        }
    });
    action
}

/// La línea grabada a lo ancho (`edge`: hasta los bordes del panel del dock).
fn etched_rule(ui: &mut egui::Ui, edge: bool) {
    let Some(skin) = pixel_ui::frame_skin(ui.ctx()) else {
        ui.separator();
        return;
    };
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), pixel_ui::ETCHED_SIZE),
        egui::Sense::hover(),
    );
    let x = if edge {
        rect.x_range()
            .expand(f32::from(crate::dock::FLYOUT_MARGIN) - 1.0)
    } else {
        rect.x_range()
    };
    let ppp = ui.ctx().pixels_per_point();
    ui.painter()
        .add(pixel_ui::etched_hline(x, rect.top(), skin.colors(), ppp));
}

/// Entre las paletas y la barra de scroll.
const LOSPEC_SCROLL_GAP: f32 = 5.0;
/// Los campos de arriba, como en el diseño: el del tag y el del orden.
#[cfg(not(target_arch = "wasm32"))]
const TAG_W: f32 = 124.0;
#[cfg(not(target_arch = "wasm32"))]
const LINK_ICON: &str = "icon.lospec.link";

/// Solo escritorio: el listado de Lospec no admite peticiones desde la web (sin CORS). Tag,
/// Colors (y cuántos), Sort By, Search y el botón de la cadena (el campo del link).
#[cfg(not(target_arch = "wasm32"))]
fn search_form(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    tab: &mut LospecTab,
    requests: &mut LospecRequests,
    link_open: &mut bool,
) {
    let mut search = false;
    // Mientras carga la primera página no se puede buscar (ni con Search ni con Enter).
    let first_page_loading = tab.listing_loading() && tab.shown_of_total().0 == 0;
    let skin = crate::pixel_ui::frame_skin(ui.ctx());
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 5.0;
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
            ui.label("Tag");
            let response = match &skin {
                Some(skin) => crate::pixel_sections::text_field(
                    ui,
                    skin,
                    &mut tab.tag,
                    TAG_W,
                    "Tag",
                    "Enter a tag",
                ),
                None => ui.add(
                    egui::TextEdit::singleline(&mut tab.tag)
                        .hint_text("Enter a tag")
                        .desired_width(TAG_W),
                ),
            };
            search |= !first_page_loading
                && response.lost_focus()
                && ui.input(|i| i.key_pressed(egui::Key::Enter));
        });
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
            ui.label("Colors");
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                crate::widgets::combo(ui, "lospec_count", &format!("{:?}", tab.count_kind), |ui| {
                    for kind in [
                        CountKind::Any,
                        CountKind::Exact,
                        CountKind::Min,
                        CountKind::Max,
                    ] {
                        crate::widgets::choice(ui, &mut tab.count_kind, kind, &format!("{kind:?}"));
                    }
                });
                ui.add_enabled_ui(tab.count_kind != CountKind::Any, |ui| {
                    crate::widgets::number(ui, &mut tab.count, 1..=256, "");
                });
            });
        });
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
            ui.label("Sort By");
            crate::widgets::combo(ui, "lospec_sort", &format!("{:?}", tab.sort), |ui| {
                for sort in [
                    LospecSort::Default,
                    LospecSort::Alphabetical,
                    LospecSort::Downloads,
                    LospecSort::Newest,
                ] {
                    crate::widgets::choice(ui, &mut tab.sort, sort, &format!("{sort:?}"));
                }
            });
        });
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
            ui.label(" ");
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 5.0;
                if first_page_loading {
                    ui.spinner();
                } else {
                    search |= crate::pixel_ui::text_button(ui, "Search").clicked();
                }
                if crate::pixel_ui::icon_button(
                    ui,
                    skin.as_deref(),
                    LINK_ICON,
                    "Paste a Lospec link",
                    Some(*link_open),
                )
                .clicked()
                {
                    *link_open = !*link_open;
                }
            });
        });
    });
    if search {
        start_search(ctx, tab, requests);
    }
}

/// Una búsqueda nueva: reemplaza las peticiones guardadas (la pestaña descarta las viejas).
#[cfg(not(target_arch = "wasm32"))]
fn start_search(ctx: &egui::Context, tab: &mut LospecTab, requests: &mut LospecRequests) {
    let start = tab.start_search();
    let now = ctx.input(|i| i.time);
    requests.listing = Some(InFlight::new(
        start.generation,
        now,
        lospec_client::fetch_listing(ctx, &start.listing),
    ));
    requests.exact = start.exact.map(|slug| {
        InFlight::new(
            start.generation,
            now,
            lospec_client::fetch_palette(ctx, &slug),
        )
    });
}

/// Los colores del diseño (frame 8): el borde de la tira de colores y lo chico del autor.
const SWATCH_BORDER: egui::Color32 = egui::Color32::from_rgb(189, 164, 153);
const SWATCH: f32 = 16.0;

/// Una paleta: el título (clic: su página en lospec.com) y "CREATED BY" el autor; a la derecha
/// sus colores, likes y descargas. Debajo la tira de colores y ◀ 1/5 ▶, el ejemplo a todo el
/// ancho y abajo Apply y Save.
fn lospec_row(
    ui: &mut egui::Ui,
    p: &LospecPalette,
    exact: bool,
    in_library: bool,
    action: &mut Option<LospecAction>,
    #[cfg(not(target_arch = "wasm32"))] examples: &mut ExampleCache,
    #[cfg(not(target_arch = "wasm32"))] now: f64,
) {
    let skin = pixel_ui::frame_skin(ui.ctx());
    let text = ui.visuals().text_color();
    let ppp = ui.ctx().pixels_per_point();
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        let title = ui.add(egui::Label::new(&p.title).sense(egui::Sense::click()));
        if let Some(author) = p.author.as_deref().filter(|a| !a.is_empty()) {
            ui.label(
                egui::RichText::new(format!("CREATED BY {}", author.to_uppercase()))
                    .small()
                    .color(text),
            );
        }
        // El link solo se abre con un slug validado: la URL va a `explorer`/`open`/`xdg-open`.
        if cs_io::lospec::is_valid_slug(&p.slug) {
            let url = cs_io::lospec::page_url(&p.slug);
            let title = title
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .tip(&url);
            if title.clicked()
                && let Err(e) = platform::open_url(ui.ctx(), &url)
            {
                log::warn!("could not open {url}: {e}");
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 1.0;
            let stat = |ui: &mut egui::Ui, icon: &str, plain: &str, value: String| {
                ui.label(value);
                match skin
                    .as_ref()
                    .and_then(|s| s.piece(icon, crate::skin::WidgetState::Normal))
                {
                    Some(piece) => {
                        let size = egui::vec2(piece.size[0] as f32, piece.size[1] as f32);
                        let (rect, _) = ui.allocate_exact_size(
                            egui::vec2(size.x, ui.spacing().interact_size.y.min(13.0)),
                            egui::Sense::hover(),
                        );
                        let at = egui::Rect::from_min_size(
                            egui::pos2(rect.left(), (rect.center().y - size.y / 2.0).round()),
                            size,
                        );
                        ui.painter().extend(pixel_ui::piece_shapes(piece, at, ppp));
                    }
                    None => {
                        ui.label(plain);
                    }
                }
                ui.add_space(5.0);
            };
            if let Some(d) = p.downloads {
                stat(ui, "icon.lospec.downloads", "⬇", d.to_string());
            }
            if let Some(l) = p.likes {
                stat(ui, "icon.lospec.likes", "♥", l.to_string());
            }
            stat(
                ui,
                "icon.lospec.colors",
                "",
                format!("{} colors", p.palette.len()),
            );
        });
    });
    ui.horizontal_top(|ui| {
        swatches(ui, &p.palette);
        #[cfg(not(target_arch = "wasm32"))]
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
            lospec_examples::pager(ui, p, examples);
        });
    });
    #[cfg(not(target_arch = "wasm32"))]
    lospec_examples::image(ui, p, examples, now);
    // Abajo: lo que se sabe de ella y Apply / Save.
    ui.horizontal(|ui| {
        let mut notes = Vec::new();
        if exact {
            notes.push("Exact match");
        }
        if in_library {
            notes.push("In library");
        }
        if !notes.is_empty() {
            ui.weak(notes.join(" · "));
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 5.0;
            if crate::pixel_ui::text_button(ui, "Save").clicked() {
                *action = Some(LospecAction::Save(p.clone()));
            }
            if crate::pixel_ui::text_button(ui, "Apply").clicked() {
                *action = Some(LospecAction::Apply(p.clone()));
            }
        });
    });
    ui.add_space(6.0);
}

/// La tira de colores en su caja (borde y 1 px blanco), de 16×16 cada uno; las que no caben a lo
/// ancho siguen abajo.
fn swatches(ui: &mut egui::Ui, palette: &Palette) {
    use eframe::emath::GuiRounding as _;
    let colors = palette.colors();
    if colors.is_empty() {
        return;
    }
    let room = (ui.available_width() * 0.6).max(SWATCH * 4.0 + 4.0);
    let per_row = (((room - 4.0) / SWATCH).floor() as usize).clamp(1, colors.len());
    let rows = colors.len().div_ceil(per_row);
    let size = egui::vec2(per_row as f32 * SWATCH + 4.0, rows as f32 * SWATCH + 4.0);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let ppp = ui.ctx().pixels_per_point();
    let painter = ui.painter();
    painter.rect_filled(rect.round_to_pixels(ppp), 0.0, SWATCH_BORDER);
    painter.rect_filled(
        rect.shrink(1.0).round_to_pixels(ppp),
        0.0,
        egui::Color32::WHITE,
    );
    for (k, &c) in colors.iter().enumerate() {
        let (row, col) = (k / per_row, k % per_row);
        let at = egui::Rect::from_min_size(
            rect.min + egui::vec2(2.0 + col as f32 * SWATCH, 2.0 + row as f32 * SWATCH),
            egui::Vec2::splat(SWATCH),
        );
        painter.rect_filled(at.round_to_pixels(ppp), 0.0, panels::to_color32(c));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_format_follows_the_extension() {
        assert_eq!(save_format("a.hex"), PaletteFormat::Hex);
        assert_eq!(save_format("a.PAL"), PaletteFormat::Pal);
        assert_eq!(save_format("a.gpl"), PaletteFormat::Gpl);
        assert_eq!(save_format("a"), PaletteFormat::Gpl);
        assert_eq!(save_format("a.png"), PaletteFormat::Gpl);
    }

    #[test]
    fn unknown_save_extensions_become_gpl() {
        assert_eq!(save_file_name("a.hex", "a"), "a.hex");
        assert_eq!(save_file_name("a.PAL", "a"), "a.PAL");
        assert_eq!(save_file_name("a.png", "a"), "a.gpl");
        assert_eq!(save_file_name("a", "a"), "a.gpl");
    }

    #[test]
    fn search_matches_case_insensitive_substrings() {
        assert!(matches_search("Sweetie 16", "sweet"));
        assert!(matches_search("Sweetie 16", ""));
        assert!(!matches_search("PICO-8", "nes"));
    }

    fn lospec_palette(name: &str, slug: &str) -> LospecPalette {
        let json = format!(r#"{{"name":"{name}","author":"","colors":["000000"]}}"#);
        cs_io::lospec::parse_palette(json.as_bytes(), slug).unwrap()
    }

    #[test]
    fn requests_poll_routes_each_response_with_its_generation() {
        let mut tab = LospecTab::default();
        tab.tag = "sweetie 16".into();
        tab.link = "pico-8".into();
        tab.start_search();
        // Generaciones distintas (búsqueda 2, lookup 1): un cruce se descartaría.
        let search = tab.start_search();
        let (lookup_g, _) = tab.start_lookup().unwrap();
        assert_ne!(search.generation, lookup_g);
        let page = LospecPage {
            palettes: vec![lospec_palette("A", "a")],
            total: 5,
            raw_count: 1,
        };
        let mut requests = LospecRequests {
            listing: Some(InFlight::new(
                search.generation,
                0.0,
                Pending::ready(Ok(page)),
            )),
            exact: Some(InFlight::new(
                search.generation,
                0.0,
                Pending::ready(Ok(lospec_palette("Sweetie 16", "sweetie-16"))),
            )),
            lookup: Some(InFlight::new(
                lookup_g,
                0.0,
                Pending::ready(Ok(lospec_palette("PICO-8", "pico-8"))),
            )),
        };
        requests.poll(&mut tab, 0.0);
        assert!(requests.listing.is_none());
        assert!(requests.exact.is_none());
        assert!(requests.lookup.is_none());
        assert!(!tab.listing_loading());
        assert!(!tab.lookup_loading());
        assert_eq!(tab.shown_of_total(), (1, 5));
        assert!(tab.is_exact("sweetie-16"));
        assert_eq!(tab.results().len(), 2);
        assert_eq!(tab.lookup_result().unwrap().slug, "pico-8");
    }

    #[test]
    fn stalled_requests_time_out() {
        let mut tab = LospecTab::default();
        tab.tag = "sweetie 16".into();
        tab.link = "pico-8".into();
        let search = tab.start_search();
        let (lookup_g, _) = tab.start_lookup().unwrap();
        let start = 100.0;
        let mut requests = LospecRequests {
            listing: Some(InFlight::new(search.generation, start, Pending::never())),
            exact: Some(InFlight::new(search.generation, start, Pending::never())),
            lookup: Some(InFlight::new(lookup_g, start, Pending::never())),
        };
        requests.poll(&mut tab, start + 10.0);
        assert!(requests.is_busy());
        assert!(tab.listing_loading());
        assert!(tab.lookup_loading());
        assert_eq!(tab.error(), None);
        requests.poll(&mut tab, start + 31.0);
        assert!(!requests.is_busy());
        assert!(!tab.listing_loading());
        assert!(!tab.lookup_loading());
        assert_eq!(tab.error(), Some("Couldn't reach Lospec."));
    }

    #[test]
    fn row_actions_save_with_lospec_comments() {
        let p = cs_io::lospec::parse_palette(
            br#"{"name":"Ice Cream GB","author":"Kerrie Lake","colors":["7c3f58","fff6d3"]}"#,
            "ice-cream-gb",
        )
        .unwrap();
        let (named, comments) = lospec_preset(&p);
        assert_eq!(named.name, "Ice Cream GB");
        assert_eq!(comments[0], "Author: Kerrie Lake");
        assert_eq!(
            comments[1],
            "Source: https://lospec.com/palette-list/ice-cream-gb"
        );
    }
}
