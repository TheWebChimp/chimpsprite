use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use cs_commands::{Command, DocCommand, History, sprite_ops};
use cs_core::{
    AnimDirection, Artboard, ArtboardId, ColorMode, Document, LayerId, Mask, ObjectId, PixelValue,
    Playback, Rgba, Sprite, Tile,
};
use cs_render::OnionSkin;
use cs_tools::text::TextStyle;
use cs_tools::{
    ArtboardTool, Blur, BrushShape, Bucket, Contour, Curve, Dither, Eraser, Eyedropper,
    GradientShape, GradientTool, Jumble, Lasso, MagicWand, Marquee, MarqueeShape, MoveTool, Pencil,
    Point, Polygon, PolygonLasso, ShapeKind, ShapeTool, SliceTool, Spray, Tool, ToolContext,
    ToolOutput,
};

use crate::tile_ops::{self, TileProxy};
use crate::transform::{Floating, TransformDrag};
use crate::transform_box::TransformHit;

mod tool_options;
pub use tool_options::*;

mod document_ops;
mod gestures;

/// Herramientas que trabajan dentro de un sprite.
struct SpriteTools {
    rect_select: Marquee,
    ellipse_select: Marquee,
    lasso: Lasso,
    polygon_lasso: PolygonLasso,
    magic_wand: MagicWand,
    pencil: Pencil,
    spray: Spray,
    eraser: Eraser,
    bucket: Bucket,
    gradient: GradientTool,
    line: ShapeTool,
    curve: Curve,
    rectangle: ShapeTool,
    ellipse: ShapeTool,
    polygon: Polygon,
    jumble: Jumble,
    contour: Contour,
    blur: Blur,
    slice: SliceTool,
    mover: MoveTool,
    eyedropper: Eyedropper,
}

impl SpriteTools {
    fn new() -> Self {
        Self {
            rect_select: Marquee::new(MarqueeShape::Rectangle),
            ellipse_select: Marquee::new(MarqueeShape::Ellipse),
            lasso: Lasso::new(),
            polygon_lasso: PolygonLasso::new(),
            magic_wand: MagicWand::new(),
            pencil: Pencil::new(),
            spray: Spray::new(),
            eraser: Eraser::new(),
            bucket: Bucket::new(),
            gradient: GradientTool::new(),
            line: ShapeTool::new(ShapeKind::Line),
            curve: Curve::new(),
            rectangle: ShapeTool::new(ShapeKind::Rectangle),
            ellipse: ShapeTool::new(ShapeKind::Ellipse),
            polygon: Polygon::new(),
            jumble: Jumble::new(),
            contour: Contour::new(),
            blur: Blur::new(),
            slice: SliceTool::new(),
            mover: MoveTool::new(),
            eyedropper: Eyedropper::new(),
        }
    }

    fn get(&self, kind: ToolKind) -> Option<&dyn Tool> {
        Some(match kind {
            ToolKind::RectSelect => &self.rect_select,
            ToolKind::EllipseSelect => &self.ellipse_select,
            ToolKind::Lasso => &self.lasso,
            ToolKind::PolygonLasso => &self.polygon_lasso,
            ToolKind::MagicWand => &self.magic_wand,
            ToolKind::Pencil => &self.pencil,
            ToolKind::Spray => &self.spray,
            ToolKind::Eraser => &self.eraser,
            ToolKind::Bucket => &self.bucket,
            ToolKind::Gradient => &self.gradient,
            ToolKind::Line => &self.line,
            ToolKind::Curve => &self.curve,
            ToolKind::Rectangle => &self.rectangle,
            ToolKind::Ellipse => &self.ellipse,
            ToolKind::Polygon => &self.polygon,
            ToolKind::Jumble => &self.jumble,
            ToolKind::Slice => &self.slice,
            ToolKind::Move => &self.mover,
            ToolKind::Eyedropper => &self.eyedropper,
            ToolKind::Contour => &self.contour,
            ToolKind::Blur => &self.blur,
            // Text y Artboard no son herramientas de sprite; Hand y Zoom las maneja el lienzo.
            ToolKind::Text | ToolKind::Artboard | ToolKind::Hand | ToolKind::Zoom => return None,
        })
    }

    /// Deja la herramienta `kind` como recién creada: descarta un trazo a medias aunque la
    /// herramienta no sepa cancelarse (`Tool::cancel` no hace nada en las de pincel).
    fn reset(&mut self, kind: ToolKind) {
        match kind {
            ToolKind::RectSelect => self.rect_select = Marquee::new(MarqueeShape::Rectangle),
            ToolKind::EllipseSelect => self.ellipse_select = Marquee::new(MarqueeShape::Ellipse),
            ToolKind::Lasso => self.lasso = Lasso::new(),
            ToolKind::PolygonLasso => self.polygon_lasso = PolygonLasso::new(),
            ToolKind::MagicWand => self.magic_wand = MagicWand::new(),
            ToolKind::Pencil => self.pencil = Pencil::new(),
            ToolKind::Spray => self.spray = Spray::new(),
            ToolKind::Eraser => self.eraser = Eraser::new(),
            ToolKind::Bucket => self.bucket = Bucket::new(),
            ToolKind::Gradient => self.gradient = GradientTool::new(),
            ToolKind::Line => self.line = ShapeTool::new(ShapeKind::Line),
            ToolKind::Curve => self.curve = Curve::new(),
            ToolKind::Rectangle => self.rectangle = ShapeTool::new(ShapeKind::Rectangle),
            ToolKind::Ellipse => self.ellipse = ShapeTool::new(ShapeKind::Ellipse),
            ToolKind::Polygon => self.polygon = Polygon::new(),
            ToolKind::Jumble => self.jumble = Jumble::new(),
            ToolKind::Slice => self.slice = SliceTool::new(),
            ToolKind::Move => self.mover = MoveTool::new(),
            ToolKind::Eyedropper => self.eyedropper = Eyedropper::new(),
            ToolKind::Contour => self.contour = Contour::new(),
            ToolKind::Blur => self.blur = Blur::new(),
            ToolKind::Text | ToolKind::Artboard | ToolKind::Hand | ToolKind::Zoom => {}
        }
    }

    fn get_mut(&mut self, kind: ToolKind) -> Option<&mut dyn Tool> {
        Some(match kind {
            ToolKind::RectSelect => &mut self.rect_select,
            ToolKind::EllipseSelect => &mut self.ellipse_select,
            ToolKind::Lasso => &mut self.lasso,
            ToolKind::PolygonLasso => &mut self.polygon_lasso,
            ToolKind::MagicWand => &mut self.magic_wand,
            ToolKind::Pencil => &mut self.pencil,
            ToolKind::Spray => &mut self.spray,
            ToolKind::Eraser => &mut self.eraser,
            ToolKind::Bucket => &mut self.bucket,
            ToolKind::Gradient => &mut self.gradient,
            ToolKind::Line => &mut self.line,
            ToolKind::Curve => &mut self.curve,
            ToolKind::Rectangle => &mut self.rectangle,
            ToolKind::Ellipse => &mut self.ellipse,
            ToolKind::Polygon => &mut self.polygon,
            ToolKind::Jumble => &mut self.jumble,
            ToolKind::Slice => &mut self.slice,
            ToolKind::Move => &mut self.mover,
            ToolKind::Eyedropper => &mut self.eyedropper,
            ToolKind::Contour => &mut self.contour,
            ToolKind::Blur => &mut self.blur,
            // Text y Artboard no son herramientas de sprite; Hand y Zoom las maneja el lienzo.
            ToolKind::Text | ToolKind::Artboard | ToolKind::Hand | ToolKind::Zoom => return None,
        })
    }
}

/// Refleja un `DrawPixels` según la simetría; otros comandos pasan igual.
fn mirror(cmd: Command, symmetry: cs_core::Symmetry, center2: (i32, i32)) -> Command {
    match cmd {
        Command::DrawPixels {
            layer,
            frame,
            pixels,
        } if symmetry.axes.any() => Command::DrawPixels {
            layer,
            frame,
            pixels: cs_raster::symmetrize(&pixels, symmetry.axes, center2),
        },
        other => other,
    }
}

/// Recorta un `DrawPixels` a la máscara; otros comandos pasan igual.
fn clip(cmd: Command, mask: &Mask) -> Command {
    match cmd {
        Command::DrawPixels {
            layer,
            frame,
            pixels,
        } => Command::DrawPixels {
            layer,
            frame,
            pixels: pixels
                .into_iter()
                .filter(|&(x, y, _)| mask.contains(x, y))
                .collect(),
        },
        other => other,
    }
}

/// Lo que el lienzo debe mostrar en este frame.
pub struct DocView<'a> {
    /// El documento, con el gesto en curso aplicado si lo hay.
    pub document: Cow<'a, Document>,
    pub selection: Option<Mask>,
}

/// Qué hace un trazo en modo Tiles.
enum TilesEdit {
    Apply(Command),
    /// Nada cambia (o la capa está bloqueada).
    Nothing,
    /// La grilla pasaría el límite de tamaño de `ops::set_tiles`.
    TooLarge,
}

/// Lo que edita un gesto agrupado: artboard, capa, frame, slice seleccionado y objeto actual.
type MergeTarget = (ArtboardId, LayerId, usize, Option<usize>, Option<ObjectId>);

/// El pincel personalizado con los valores del sprite, compartido con cada contexto de las
/// herramientas; y para qué versión del pincel, artboard y documento se armó.
type BrushImage = std::rc::Rc<[((i32, i32), PixelValue)]>;
type BrushImageKey = (u64, ArtboardId, u64);

/// View → Tiled Mode, como en Aseprite: en qué ejes se repite el artboard activo.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TiledMode {
    #[default]
    None,
    Both,
    X,
    Y,
}

impl TiledMode {
    pub const ALL: [TiledMode; 4] = [TiledMode::None, TiledMode::Both, TiledMode::X, TiledMode::Y];

    pub fn label(self) -> &'static str {
        match self {
            TiledMode::None => "None",
            TiledMode::Both => "Tiled in Both Axes",
            TiledMode::X => "Tiled in X Axis",
            TiledMode::Y => "Tiled in Y Axis",
        }
    }

    /// Se repite a lo ancho.
    pub fn x(self) -> bool {
        matches!(self, TiledMode::Both | TiledMode::X)
    }

    /// Se repite a lo alto.
    pub fn y(self) -> bool {
        matches!(self, TiledMode::Both | TiledMode::Y)
    }
}

/// Estado del documento abierto y de la edición. No sabe nada de egui.
pub struct Editor {
    pub document: Document,
    pub history: History<DocCommand>,
    pub active: ArtboardId,
    pub layer: LayerId,
    pub frame: usize,
    pub primary: PixelValue,
    pub secondary: PixelValue,
    pub tool: ToolKind,
    pub options: ToolOptions,
    /// Selección dentro del artboard activo.
    pub selection: Option<Mask>,
    /// Aumenta cada vez que el documento o la selección cambian.
    pub revision: u64,
    /// Capa y frame recordados por artboard.
    views: HashMap<ArtboardId, (LayerId, usize)>,
    tools: SpriteTools,
    artboard_tool: ArtboardTool,
    painting_secondary: bool,
    /// Alt al presionar: cuentagotas temporal durante el gesto.
    temp_eyedropper: bool,
    /// Reproducción en curso.
    pub playing: bool,
    /// Estado de la reproducción: posición, rango (inclusivo) y dirección.
    pub(crate) playback: Option<(Playback, (usize, usize), AnimDirection)>,
    /// Milisegundos acumulados en el frame actual durante la reproducción.
    pub(crate) play_clock: f64,
    /// Rango de frames seleccionado en el timeline (para tags, duraciones…).
    pub frame_range: Option<(usize, usize)>,
    /// Las capas del rango de cels (de la capa donde empezó a la otra punta, en el orden del
    /// timeline) y los frames para los que se eligió: vale mientras `frame_range` sea esos.
    pub layer_range: Option<(LayerId, LayerId, (usize, usize))>,
    /// Los frames se eligieron en el encabezado del timeline: copiar y pegar es de frames
    /// enteros (si no, de cels).
    pub frames_picked: bool,
    /// Grupos colapsados en el timeline (estado de UI, por artboard).
    pub collapsed: HashSet<(ArtboardId, LayerId)>,
    pub onion_enabled: bool,
    /// View → Tiled Mode: el artboard activo se ve repetido y lo que se pinta en una copia cae en
    /// el sprite (para texturas que se repiten sin costura).
    pub tiled: TiledMode,
    pub onion: OnionSkin,
    /// Cambia con cada modificación del documento (ejecutar, deshacer, rehacer); sirve para
    /// saber si hay cambios sin guardar.
    pub doc_version: u64,
    /// A qué se pegan las herramientas de forma (lo actualiza el lienzo cada frame).
    pub snap: Option<crate::guides::SnapContext>,
    /// El pincel personalizado con sus colores ya pasados al sprite, y para qué lo estaba: la
    /// versión del pincel, el artboard y la del documento (ver `brush_image`).
    brush_image: std::cell::RefCell<Option<(BrushImageKey, BrushImage)>>,
    /// Última selección descartada (para Reselect).
    last_selection: Option<Mask>,
    /// Modo de combinación del gesto de selección en curso.
    gesture_mode: SelectionMode,
    /// Shift ya estaba presionado al empezar el gesto (se usó para el modo, no para restringir).
    shift_at_press: bool,
    /// Texto en edición con la herramienta Text.
    pub text: Option<TextDraft>,
    /// Las herramientas pintan en la máscara de la capa actual (si tiene).
    pub edit_mask: bool,
    /// Slice seleccionado del artboard activo (índice en `sprite.slices`).
    pub slice: Option<usize>,
    pub text_style: TextStyle,
    /// Gesto en curso (p. ej. arrastrar un campo del panel) cuyos cambios se juntan en un solo
    /// paso de undo: id del widget y lo que se estaba editando (artboard, capa, frame, slice).
    /// El mismo widget sirve para todas las capas, así que el objetivo también tiene que coincidir.
    merge: Option<(u64, MergeTarget)>,
    /// Tile con el que pintan las herramientas en modo Tiles (índice y flips). Ver `brush_tile()`.
    pub(crate) brush_tile: Tile,
    /// Artboard y tileset para los que se eligió `brush_tile`; en otro tileset el pincel es el 1.
    pub(crate) brush_tileset: Option<(ArtboardId, u32)>,
    /// El último trazo en modo Tiles no se aplicó porque la grilla pasaría el límite de tamaño.
    pub(crate) tile_limit: bool,
    /// Contenido levantado que se está moviendo, escalando o rotando (ver `transform.rs`).
    pub(crate) floating: Option<Floating>,
    /// Arrastre en curso para mover, escalar o rotar lo seleccionado.
    pub(crate) transform_drag: Option<TransformDrag>,
    /// Con la paleta bloqueada (lo normal), sus colores no se editan uno por uno: el editor de
    /// color cambia el color primario y nada más. Ver `palette_entry_of`.
    pub palette_locked: bool,
    /// La entrada de la paleta elegida con un clic en su cuadrito (Pixel UI) para cada color:
    /// `[primario, secundario]`. Con la paleta desbloqueada, el editor de color edita la del
    /// color que edita, mientras el color siga siendo el suyo. Ver `pick_color`.
    pub palette_entry: [Option<u8>; 2],
    /// Los cuadritos de la paleta seleccionados (Pixel UI): de dónde empezó (con un clic) a dónde
    /// llega (con Shift+clic o arrastrando). Ver `palette_range`.
    pub palette_selection: Option<(usize, usize)>,
    /// El color primario de antes de elegir el Mask con su botón, para volver al apagarlo.
    pub(crate) before_mask: Option<PixelValue>,
    /// Con Pixel UI, el editor de color edita el secundario: se elige con un clic en la mitad
    /// de abajo de la caja de los colores, o al elegir un color con clic derecho (`pick_color`).
    /// Sin Pixel UI vuelve a `false` (`ChimpApp::layout`). Ver `edited_color`.
    pub editing_secondary: bool,
    /// El símbolo elegido en el panel de la biblioteca (o el último creado con F8).
    pub library_selected: Option<ArtboardId>,
    /// El modo símbolo: los símbolos de la biblioteca en edición, del de afuera al de adentro.
    /// Vacía: se edita el documento. Si no, el activo es el último.
    pub symbol_stack: Vec<ArtboardId>,
    /// El artboard activo al entrar al modo símbolo desde el documento (para volver).
    pub symbol_return: Option<ArtboardId>,
    /// Los objetos seleccionados, en el orden en que se eligieron, y de qué artboard y capa son
    /// (los ids de objeto son por capa). Se leen con `selected_objects()`, que no devuelve nada
    /// si la capa actual es otra. Ver `object_ops.rs`.
    object_selection: Vec<ObjectId>,
    object_selection_at: Option<(ArtboardId, LayerId)>,
    /// El gesto de Move en curso empezó con Alt sobre un objeto: mueve copias.
    pub(crate) object_copy: bool,
    /// Las cajas de los objetos ya calculadas (ver `object_bounds`), para qué `revision`.
    pub(crate) object_bounds_cache: std::cell::RefCell<(u64, ObjectBoundsCache)>,
}

/// Caja `(x, y, ancho, alto)` de un objeto por artboard, frame, capa y objeto.
pub(crate) type ObjectBoundsCache =
    HashMap<(ArtboardId, usize, LayerId, ObjectId), Option<(i32, i32, u32, u32)>>;

impl Editor {
    pub fn new(document: Document) -> Self {
        assert!(
            !document.artboards.is_empty(),
            "a document needs at least one artboard"
        );
        // Empieza en el primer artboard del lienzo (la biblioteca no se ve).
        let active = document
            .canvas_artboards()
            .next()
            .unwrap_or(&document.artboards[0])
            .id;
        let mut editor = Self {
            document,
            history: History::new(),
            active,
            layer: LayerId(1),
            frame: 0,
            primary: PixelValue::Index(0),
            secondary: PixelValue::Index(0),
            tool: ToolKind::Pencil,
            options: ToolOptions::default(),
            selection: None,
            revision: 0,
            views: HashMap::new(),
            tools: SpriteTools::new(),
            artboard_tool: ArtboardTool::new(),
            painting_secondary: false,
            temp_eyedropper: false,
            playing: false,
            playback: None,
            play_clock: 0.0,
            frame_range: None,
            layer_range: None,
            frames_picked: false,
            collapsed: HashSet::new(),
            onion_enabled: false,
            tiled: TiledMode::None,
            onion: OnionSkin::default(),
            doc_version: 0,
            snap: None,
            brush_image: Default::default(),
            last_selection: None,
            gesture_mode: SelectionMode::Replace,
            shift_at_press: false,
            text: None,
            text_style: TextStyle::default(),
            merge: None,
            edit_mask: false,
            slice: None,
            brush_tile: Tile::new(1, false, false, false),
            brush_tileset: None,
            tile_limit: false,
            floating: None,
            transform_drag: None,
            palette_locked: true,
            palette_entry: [None; 2],
            palette_selection: None,
            before_mask: None,
            editing_secondary: false,
            library_selected: None,
            symbol_stack: Vec::new(),
            symbol_return: None,
            object_selection: Vec::new(),
            object_selection_at: None,
            object_bounds_cache: Default::default(),
            object_copy: false,
        };
        editor.ensure_valid();
        // Negro para pintar (el más cercano de la paleta, si es indexado) y la máscara de
        // secundario, para borrar con el clic derecho.
        let black = Rgba::rgb(0, 0, 0);
        let sprite = editor.sprite();
        editor.primary = match sprite.color_mode {
            ColorMode::Indexed { transparent_index } => {
                PixelValue::Index(sprite.palette.nearest(black, transparent_index))
            }
            ColorMode::Rgba | ColorMode::Grayscale => PixelValue::Rgba(black),
        };
        editor.secondary = editor.mask_value();
        editor
    }

    pub fn with_sprite(sprite: Sprite) -> Self {
        Self::new(Document::with_sprite("Artboard 1", sprite))
    }

    pub fn artboard(&self) -> &Artboard {
        self.document
            .artboard(self.active)
            .expect("active artboard exists")
    }

    pub fn sprite(&self) -> &Sprite {
        &self.artboard().sprite
    }

    /// Mantiene invariantes tras cambios en el documento (p. ej. undo de un artboard).
    pub(crate) fn ensure_valid(&mut self) {
        // La pila del modo símbolo se corta en el primero que ya no es un símbolo de la
        // biblioteca (p. ej. un undo lo borró o lo sacó al lienzo).
        let document = &self.document;
        let symbol = |id: &ArtboardId| document.artboard(*id).is_some_and(|a| a.in_library);
        if let Some(cut) = self.symbol_stack.iter().position(|id| !symbol(id)) {
            self.symbol_stack.truncate(cut);
        }
        let target = match self.symbol_stack.last() {
            // En el modo símbolo, el activo es el último de la pila.
            Some(&last) => Some(last),
            // Fuera de él, el activo es uno del lienzo: el de antes de entrar o el primero.
            None => {
                let back = self.symbol_return.take();
                (!self.on_canvas(self.active)).then(|| {
                    back.filter(|&id| self.on_canvas(id)).unwrap_or_else(|| {
                        self.document
                            .canvas_artboards()
                            .next()
                            .unwrap_or(&self.document.artboards[0])
                            .id
                    })
                })
            }
        };
        if let Some(id) = target.filter(|&id| id != self.active) {
            if self.document.artboard(self.active).is_some() {
                // El de antes sigue ahí: el cambio completo (confirma lo flotante, cierra el
                // grupo y recuerda su capa y su frame). `activate` vuelve a validar.
                self.activate(id);
                return;
            }
            self.active = id;
            if let Some(&(layer, frame)) = self.views.get(&id) {
                (self.layer, self.frame) = (layer, frame);
            }
            self.selection = None;
            self.slice = None;
        }
        let sprite = &self
            .document
            .artboard(self.active)
            .expect("active artboard exists")
            .sprite;
        if sprite.layer(self.layer).is_none() {
            self.layer = sprite.layers.last().map_or(LayerId(1), |l| l.id);
        }
        self.frame = self.frame.min(sprite.frames.len().saturating_sub(1));
        let slices = sprite.slices.len();
        self.slice = self.slice.filter(|&i| i < slices);
        // La selección de objetos es de una capa: si la actual es otra se descarta, y si no, se
        // quitan los objetos que ya no están (un undo).
        match sprite.layer(self.layer) {
            Some(layer) if self.object_selection_at == Some((self.active, self.layer)) => {
                self.object_selection.retain(|&o| layer.object(o).is_some())
            }
            _ => {
                self.object_selection.clear();
                self.object_selection_at = None;
            }
        }
        let last = sprite.frames.len().saturating_sub(1);
        self.frame_range = self.frame_range.map(|(a, b)| (a.min(last), b.min(last)));
        let (w, h) = (sprite.width, sprite.height);
        if self
            .selection
            .as_ref()
            .is_some_and(|m| (m.width(), m.height()) != (w, h))
        {
            self.selection = None;
        }
    }

    /// Si `id` es un artboard del lienzo (no de la biblioteca).
    fn on_canvas(&self, id: ArtboardId) -> bool {
        self.document.artboard(id).is_some_and(|a| !a.in_library)
    }

    /// Activa otro artboard: recuerda capa/frame del actual y descarta la selección.
    pub fn activate(&mut self, id: ArtboardId) {
        if id == self.active || self.document.artboard(id).is_none() {
            return;
        }
        // Activar algo fuera de la pila (el panel de Artboards, un artboard nuevo) sale del modo
        // símbolo.
        if !self.symbol_stack.contains(&id) {
            self.symbol_stack.clear();
            self.symbol_return = None;
        }
        self.commit_floating();
        self.stop();
        self.end_group();
        self.frame_range = None;
        self.views.insert(self.active, (self.layer, self.frame));
        self.active = id;
        (self.layer, self.frame) = self.views.get(&id).copied().unwrap_or((LayerId(0), 0));
        self.selection = None;
        self.slice = None;
        self.ensure_valid();
        self.revision += 1;
    }

    /// Valor de píxel para la entrada `index` de la paleta según el modo de color.
    pub fn palette_value(&self, index: u8) -> PixelValue {
        let sprite = self.sprite();
        match sprite.color_mode {
            ColorMode::Indexed { .. } => PixelValue::Index(index),
            ColorMode::Rgba | ColorMode::Grayscale => {
                PixelValue::Rgba(sprite.palette.get(index).unwrap_or_default())
            }
        }
    }

    pub fn resolve(&self, value: PixelValue) -> Rgba {
        cs_render::resolve(self.sprite(), value)
    }

    /// Ejecuta un comando sobre el artboard activo; devuelve si se aplicó.
    /// Los objetos seleccionados de la capa actual (vacío si la selección era de otra capa).
    pub fn selected_objects(&self) -> &[ObjectId] {
        if self.object_selection_at == Some((self.active, self.layer)) {
            &self.object_selection
        } else {
            &[]
        }
    }

    /// Reemplaza la selección de objetos por `ids`, de la capa actual.
    pub(crate) fn set_selected_objects(&mut self, ids: Vec<ObjectId>) {
        self.object_selection = ids;
        self.object_selection_at = Some((self.active, self.layer));
    }

    pub fn execute(&mut self, cmd: Command) -> bool {
        self.execute_doc(DocCommand::OnArtboard {
            id: self.active,
            cmd,
        })
    }

    /// Como `execute`, con `label` como nombre del paso en el historial (el de la herramienta o
    /// el del menú: "Pencil", "Fill"… en vez del del comando).
    pub fn execute_named(&mut self, cmd: Command, label: &'static str) -> bool {
        let applied = self.execute(cmd);
        if applied {
            self.history.rename_last(label);
        }
        applied
    }

    /// Corre `f` (que puede ejecutar varios comandos, o ninguno) y, si dejó pasos nuevos, le pone
    /// `label` al último. Lo flotante se confirma antes si hace falta (si no, su paso podría
    /// quedarse con el nombre).
    pub(crate) fn named<R>(&mut self, label: &'static str, f: impl FnOnce(&mut Self) -> R) -> R {
        let before = self.history.done_count();
        let result = f(self);
        if self.history.done_count() > before {
            self.history.rename_last(label);
        }
        result
    }

    /// Ejecuta un comando sobre el documento. Antes confirma lo flotante: guarda índices (capa,
    /// frame) y una vista previa que otro cambio dejaría desactualizados.
    pub fn execute_doc(&mut self, cmd: DocCommand) -> bool {
        self.commit_floating();
        self.apply_doc(cmd)
    }

    /// Ejecuta sin confirmar lo flotante (lo usa la confirmación misma).
    pub(crate) fn apply_doc(&mut self, cmd: DocCommand) -> bool {
        self.merge = None;
        match self.history.execute(&mut self.document, cmd) {
            Ok(()) => {
                self.revision += 1;
                self.doc_version += 1;
                self.ensure_valid();
                true
            }
            Err(e) => {
                log::warn!("command rejected: {e}");
                false
            }
        }
    }

    /// Como `execute`, pero los comandos seguidos del mismo `group` forman un solo paso de undo.
    /// El comando debe reemplazar un estado completo (como `SetObjects` o `SetSlices`) para
    /// que el inverso del primero deshaga todo el gesto.
    pub fn execute_grouped(&mut self, cmd: Command, group: u64) -> bool {
        // Como `execute_doc`: lo flotante se confirma antes (y corta el grupo).
        self.commit_floating();
        let cmd = DocCommand::OnArtboard {
            id: self.active,
            cmd,
        };
        let target = (
            self.active,
            self.layer,
            self.frame,
            self.slice,
            self.current_object(),
        );
        let result = if self.merge == Some((group, target)) {
            self.history.execute_merged(&mut self.document, cmd)
        } else {
            self.history.execute(&mut self.document, cmd)
        };
        match result {
            Ok(()) => {
                self.merge = Some((group, target));
                self.revision += 1;
                self.doc_version += 1;
                self.ensure_valid();
                true
            }
            Err(e) => {
                log::warn!("command rejected: {e}");
                false
            }
        }
    }

    /// Termina el gesto agrupado en curso: el próximo cambio es un paso de undo nuevo.
    pub fn end_group(&mut self) {
        self.merge = None;
    }

    /// Como `end_group`, solo si el gesto en curso es el de `group`.
    pub fn end_group_of(&mut self, group: u64) {
        if self.merge.is_some_and(|(g, _)| g == group) {
            self.merge = None;
        }
    }

    pub fn undo(&mut self) {
        // Mientras flota, deshacer cancela la transformación (no hay paso en el historial).
        if self.cancel_floating() {
            return;
        }
        self.merge = None;
        match self.history.undo(&mut self.document) {
            Ok(true) => {
                self.revision += 1;
                self.doc_version += 1;
                self.ensure_valid();
            }
            Ok(false) => {}
            Err(e) => log::error!("undo failed: {e}"),
        }
    }

    pub fn redo(&mut self) {
        // Rehacer con algo flotando lo perdería: se ignora hasta confirmar o cancelar.
        if self.floating.is_some() {
            return;
        }
        self.merge = None;
        match self.history.redo(&mut self.document) {
            Ok(true) => {
                self.revision += 1;
                self.doc_version += 1;
                self.ensure_valid();
            }
            Ok(false) => {}
            Err(e) => log::error!("redo failed: {e}"),
        }
    }

    /// Color "Mask" (transparente) del sprite: en RGBA alfa 0, en indexado el índice transparente.
    /// Pintar con él borra.
    pub fn mask_value(&self) -> PixelValue {
        self.sprite().color_mode.transparent()
    }

    pub fn is_mask(&self, value: PixelValue) -> bool {
        value == self.mask_value()
    }

    pub fn swap_colors(&mut self) {
        std::mem::swap(&mut self.primary, &mut self.secondary);
        // Cada color se lleva su cuadrito de la paleta.
        self.palette_entry.swap(0, 1);
    }

    pub fn set_selection(&mut self, selection: Option<Mask>) {
        // Cambiar la selección (deseleccionar, seleccionar todo, un gesto…) confirma lo flotante.
        self.commit_floating();
        let selection = selection.filter(|m| !m.is_empty());
        if selection.is_none()
            && let Some(old) = self.selection.take()
        {
            self.last_selection = Some(old);
        }
        self.selection = selection;
        self.revision += 1;
    }

    /// Vuelve a la última selección descartada (si sigue cabiendo en el lienzo).
    pub fn reselect(&mut self) {
        let s = self.sprite();
        let (w, h) = (s.width, s.height);
        if let Some(last) = self.last_selection.clone()
            && (last.width(), last.height()) == (w, h)
        {
            self.set_selection(Some(last));
        }
    }

    /// Invierte la selección; sin selección, selecciona todo.
    pub fn invert_selection(&mut self) {
        let s = self.sprite();
        let inverted = match &self.selection {
            Some(mask) => mask.inverted(),
            None => Mask::all(s.width, s.height),
        };
        self.set_selection(Some(inverted));
    }

    pub fn can_reselect(&self) -> bool {
        self.last_selection.is_some()
    }

    /// Cambia de herramienta descartando gestos de varios clics a medias. La herramienta queda
    /// como la elegida de su grupo en la barra.
    pub fn set_tool(&mut self, kind: ToolKind) {
        self.options.group_choice.remember(kind);
        if kind != self.tool {
            self.commit_floating();
            self.cancel_gesture();
            self.tool = kind;
        }
    }

    /// La herramienta que muestra el grupo `group` de la barra: la activa si es de ese grupo; si
    /// no, la última elegida en él.
    pub fn group_tool(&self, group: usize) -> ToolKind {
        if ToolKind::GROUPS[group].contains(&self.tool) {
            self.tool
        } else {
            self.options.group_choice.get(group)
        }
    }

    pub fn select_all(&mut self) {
        let s = self.sprite();
        self.set_selection(Some(Mask::all(s.width, s.height)));
    }

    pub fn deselect(&mut self) {
        self.set_selection(None);
    }

    /// Borra (deja transparente) el contenido seleccionado de la capa actual.
    pub fn clear_selection_content(&mut self) {
        self.named("Clear", Self::clear_content);
    }

    fn clear_content(&mut self) {
        // Mientras flota, Delete descarta lo pegado o deja solo el hueco de lo levantado.
        if self.delete_floating() {
            return;
        }
        let Some(mask) = &self.selection else {
            return;
        };
        if self.editing_mask() {
            let pixels = mask.points().map(|(x, y)| (x, y, 0)).collect();
            self.execute(Command::DrawMask {
                layer: self.layer,
                pixels,
            });
            return;
        }
        if self.tiles_mode() {
            self.clear_selected_cells();
            return;
        }
        let t = self.sprite().color_mode.transparent();
        let pixels = mask.points().map(|(x, y)| (x, y, t)).collect();
        self.execute(Command::DrawPixels {
            layer: self.layer,
            frame: self.frame,
            pixels,
        });
    }

    /// Edit → Stroke (S), como en Aseprite: pinta con el color primario el borde de la selección
    /// (sus píxeles que tocan algo de afuera, de lado), en la capa actual (un paso de undo).
    pub fn stroke_selection(&mut self) {
        self.commit_floating();
        if !self.clip_layer_ok(true) {
            return;
        }
        let Some(mask) = &self.selection else {
            return;
        };
        let color = self.primary;
        let pixels: Vec<_> = mask
            .points()
            .filter(|&(x, y)| {
                [(1, 0), (-1, 0), (0, 1), (0, -1)]
                    .iter()
                    .any(|(dx, dy)| !mask.contains(x + dx, y + dy))
            })
            .map(|(x, y)| (x, y, color))
            .collect();
        if pixels.is_empty() {
            return;
        }
        self.execute_named(
            Command::DrawPixels {
                layer: self.layer,
                frame: self.frame,
                pixels,
            },
            "Stroke",
        );
    }

    /// Punto de mundo → píxel local del artboard activo.
    /// Frame a mostrar de un artboard: el actual si es el activo; si no, el recordado.
    pub fn frame_of(&self, id: ArtboardId) -> usize {
        if id == self.active {
            self.frame
        } else {
            self.views.get(&id).map_or(0, |v| v.1)
        }
    }

    pub fn guides(&self) -> &[cs_core::Guide] {
        &self.artboard().guides
    }

    /// Reemplaza las guías del artboard activo (un paso de undo).
    pub fn set_guides(&mut self, guides: Vec<cs_core::Guide>) {
        if guides.as_slice() != self.guides() {
            self.execute_doc(DocCommand::SetGuides {
                id: self.active,
                guides,
            });
        }
    }

    pub fn symmetry(&self) -> cs_core::Symmetry {
        self.artboard().symmetry
    }

    /// Cambia la simetría del artboard activo (un paso de undo).
    pub fn set_symmetry(&mut self, symmetry: cs_core::Symmetry) {
        if symmetry != self.symmetry() {
            self.execute_doc(DocCommand::SetSymmetry {
                id: self.active,
                symmetry,
            });
        }
    }

    /// Documento a mostrar: el actual o un clon con el gesto en curso aplicado.
    pub fn view(&self) -> DocView<'_> {
        let current = || DocView {
            document: Cow::Borrowed(&self.document),
            selection: self.selection.clone(),
        };
        if self.artboard_tool.is_active() {
            let Some(cmd) = self.artboard_tool.preview(&self.document) else {
                return current();
            };
            let mut doc = self.document.clone();
            return match cmd.apply(&mut doc) {
                Ok(_) => DocView {
                    document: Cow::Owned(doc),
                    // La selección no aplica si el tamaño cambia durante el gesto.
                    selection: None,
                },
                Err(_) => current(),
            };
        }
        if let Some(view) = self.floating_view() {
            return view;
        }
        if let Some(cmd) = self.text_command() {
            let cmd = self.shape_paint(ToolKind::Text, cmd);
            let mut doc = self.document.clone();
            let applied = DocCommand::OnArtboard {
                id: self.active,
                cmd,
            }
            .apply(&mut doc);
            return DocView {
                document: if applied.is_ok() {
                    Cow::Owned(doc)
                } else {
                    Cow::Borrowed(&self.document)
                },
                selection: self.selection.clone(),
            };
        }
        let kind = self.active_tool();
        let Some(tool) = self
            .tools
            .get(kind)
            .filter(|t| t.is_active() || t.is_pending())
        else {
            return current();
        };
        if let Some(proxy) = self.cell_proxy(kind) {
            let out = tool.preview(&self.tiles_context(&proxy));
            return self.tiles_view(kind, out, &proxy);
        }
        let out = tool.preview(&self.context());
        let selection = match out.selection {
            Some(sel) => self.combine_selection(kind, sel),
            None => self.selection.clone(),
        };
        let Some(cmd) = out.command else {
            return DocView {
                document: Cow::Borrowed(&self.document),
                selection,
            };
        };
        let cmd = self.shape_paint(kind, cmd);
        let mut doc = self.document.clone();
        let applied = DocCommand::OnArtboard {
            id: self.active,
            cmd,
        }
        .apply(&mut doc);
        DocView {
            document: if applied.is_ok() {
                Cow::Owned(doc)
            } else {
                Cow::Borrowed(&self.document)
            },
            selection,
        }
    }
}

#[cfg(test)]
mod tests;
