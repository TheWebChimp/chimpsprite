//! Las herramientas (ToolKind), los modos de selección y las opciones que comparten.

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ToolKind {
    RectSelect,
    EllipseSelect,
    Lasso,
    PolygonLasso,
    MagicWand,
    Hand,
    /// Acerca con un clic y aleja con el derecho (o Alt); la maneja el lienzo.
    Zoom,
    Move,
    Pencil,
    Spray,
    Eraser,
    Eyedropper,
    Bucket,
    Gradient,
    Line,
    /// Recta que se curva con dos puntos de control.
    Curve,
    Rectangle,
    Ellipse,
    Polygon,
    Jumble,
    Text,
    Contour,
    Blur,
    Slice,
    Artboard,
}

impl ToolKind {
    /// Grupos de la barra (como en Aseprite); el primero de cada grupo es el predeterminado.
    pub const GROUPS: [&'static [ToolKind]; 13] = [
        &[
            ToolKind::RectSelect,
            ToolKind::EllipseSelect,
            ToolKind::Lasso,
            ToolKind::PolygonLasso,
            ToolKind::MagicWand,
        ],
        &[ToolKind::Hand, ToolKind::Zoom],
        &[ToolKind::Pencil, ToolKind::Spray],
        &[ToolKind::Eraser],
        &[ToolKind::Eyedropper],
        &[ToolKind::Bucket, ToolKind::Gradient, ToolKind::Jumble],
        &[ToolKind::Line, ToolKind::Curve],
        &[ToolKind::Rectangle, ToolKind::Ellipse, ToolKind::Polygon],
        &[ToolKind::Text],
        &[ToolKind::Contour],
        &[ToolKind::Blur],
        &[ToolKind::Move, ToolKind::Slice],
        &[ToolKind::Artboard],
    ];

    /// Índice en `GROUPS` del grupo que tiene a `self`.
    pub fn group(self) -> usize {
        ToolKind::GROUPS
            .iter()
            .position(|g| g.contains(&self))
            .expect("every tool is in a group")
    }

    pub fn label(self) -> &'static str {
        match self {
            ToolKind::RectSelect => "Rectangular Marquee",
            ToolKind::EllipseSelect => "Elliptical Marquee",
            ToolKind::Lasso => "Lasso",
            ToolKind::PolygonLasso => "Polygonal Lasso",
            ToolKind::MagicWand => "Magic Wand",
            ToolKind::Move => "Move",
            ToolKind::Pencil => "Pencil",
            ToolKind::Spray => "Spray",
            ToolKind::Eraser => "Eraser",
            ToolKind::Eyedropper => "Eyedropper",
            ToolKind::Bucket => "Paint Bucket",
            ToolKind::Gradient => "Gradient",
            ToolKind::Line => "Line",
            ToolKind::Curve => "Curve",
            ToolKind::Rectangle => "Rectangle",
            ToolKind::Ellipse => "Ellipse",
            ToolKind::Polygon => "Polygon",
            ToolKind::Jumble => "Jumble",
            ToolKind::Text => "Text",
            ToolKind::Slice => "Slice",
            ToolKind::Artboard => "Artboard",
            ToolKind::Hand => "Hand",
            ToolKind::Zoom => "Zoom",
            ToolKind::Contour => "Contour",
            ToolKind::Blur => "Blur",
        }
    }

    pub fn shortcut(self) -> &'static str {
        match self {
            ToolKind::RectSelect => "M",
            ToolKind::EllipseSelect => "Shift+M",
            ToolKind::Lasso => "Q",
            ToolKind::PolygonLasso => "Shift+Q",
            ToolKind::MagicWand => "W",
            ToolKind::Move => "V",
            ToolKind::Pencil => "B",
            ToolKind::Spray => "Shift+S",
            ToolKind::Eraser => "E",
            ToolKind::Eyedropper => "I",
            ToolKind::Bucket => "G",
            ToolKind::Gradient => "Shift+G",
            ToolKind::Line => "L",
            ToolKind::Curve => "Shift+L",
            ToolKind::Rectangle => "U",
            ToolKind::Ellipse => "Shift+U",
            ToolKind::Polygon => "Shift+D",
            ToolKind::Jumble => "Shift+R",
            ToolKind::Text => "T",
            ToolKind::Slice => "Shift+C",
            ToolKind::Artboard => "Shift+O",
            ToolKind::Hand => "H",
            ToolKind::Zoom => "Z",
            ToolKind::Contour => "D",
            ToolKind::Blur => "R",
        }
    }

    pub fn is_selection(self) -> bool {
        matches!(
            self,
            ToolKind::RectSelect
                | ToolKind::EllipseSelect
                | ToolKind::Lasso
                | ToolKind::PolygonLasso
                | ToolKind::MagicWand
        )
    }

    /// Herramientas que pintan y por lo tanto se recortan a la selección.
    /// Herramientas que pintan con el ink elegido (las que ponen el color del pincel).
    pub fn uses_ink(self) -> bool {
        matches!(
            self,
            ToolKind::Pencil
                | ToolKind::Spray
                | ToolKind::Bucket
                | ToolKind::Line
                | ToolKind::Curve
                | ToolKind::Rectangle
                | ToolKind::Ellipse
                | ToolKind::Polygon
                | ToolKind::Text
                | ToolKind::Contour
        )
    }

    pub fn paints(self) -> bool {
        matches!(
            self,
            ToolKind::Pencil
                | ToolKind::Spray
                | ToolKind::Eraser
                | ToolKind::Bucket
                | ToolKind::Gradient
                | ToolKind::Line
                | ToolKind::Curve
                | ToolKind::Rectangle
                | ToolKind::Ellipse
                | ToolKind::Polygon
                | ToolKind::Jumble
                | ToolKind::Text
                | ToolKind::Contour
                | ToolKind::Blur
        )
    }

    /// Herramientas que pintan con la simetría del artboard. Text no (el texto se leería al
    /// revés) y Blur tampoco (como en Aseprite).
    pub fn mirrors(self) -> bool {
        self.paints() && !matches!(self, ToolKind::Text | ToolKind::Blur)
    }

    /// Herramientas que usan el tamaño de pincel.
    pub fn uses_brush(self) -> bool {
        matches!(
            self,
            ToolKind::Pencil
                | ToolKind::Spray
                | ToolKind::Eraser
                | ToolKind::Line
                | ToolKind::Curve
                | ToolKind::Rectangle
                | ToolKind::Ellipse
                | ToolKind::Polygon
                | ToolKind::Jumble
                | ToolKind::Blur
        )
    }

    /// Herramientas que no actúan sobre una capa tilemap en modo Tiles (2A).
    pub fn blocked_in_tiles(self) -> bool {
        matches!(
            self,
            ToolKind::Spray
                | ToolKind::Jumble
                | ToolKind::Gradient
                | ToolKind::Text
                | ToolKind::Lasso
                | ToolKind::PolygonLasso
                | ToolKind::Contour
                | ToolKind::Polygon
                | ToolKind::Blur
        )
    }

    /// Herramientas que no actúan mientras se edita una máscara de capa: promedian o mezclan los
    /// colores de la capa y los escribirían como niveles de la máscara.
    pub fn blocked_on_mask(self) -> bool {
        matches!(self, ToolKind::Jumble | ToolKind::Blur)
    }

    /// Herramientas que en modo Tiles trabajan sobre el proxy de celdas (Move, Slice y Artboard
    /// siguen en píxeles).
    pub fn uses_cells(self) -> bool {
        matches!(
            self,
            ToolKind::Pencil
                | ToolKind::Eraser
                | ToolKind::Line
                | ToolKind::Curve
                | ToolKind::Rectangle
                | ToolKind::Ellipse
                | ToolKind::Bucket
                | ToolKind::Eyedropper
                | ToolKind::RectSelect
                | ToolKind::EllipseSelect
                | ToolKind::MagicWand
        )
    }
}

/// Cómo se combina una selección nueva con la existente.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SelectionMode {
    #[default]
    Replace,
    Add,
    Subtract,
    Intersect,
}

impl SelectionMode {
    pub const ALL: [SelectionMode; 4] = [
        SelectionMode::Replace,
        SelectionMode::Add,
        SelectionMode::Subtract,
        SelectionMode::Intersect,
    ];

    pub fn label(self) -> &'static str {
        match self {
            SelectionMode::Replace => "Replace",
            SelectionMode::Add => "Add (Shift)",
            SelectionMode::Subtract => "Subtract (Alt)",
            SelectionMode::Intersect => "Intersect (Shift+Alt)",
        }
    }

    /// Combina la forma `shape` de un gesto con la selección `base`. `shape = None` es un clic
    /// sin forma: en Replace deselecciona; en los demás modos no cambia nada.
    pub fn combine(self, base: Option<&Mask>, shape: Option<Mask>) -> Option<Mask> {
        match (self, shape, base) {
            (SelectionMode::Replace, shape, _) => shape,
            (_, None, base) => base.cloned(),
            (SelectionMode::Add, Some(s), Some(b)) => Some(b.union(&s)),
            (SelectionMode::Add, Some(s), None) => Some(s),
            (SelectionMode::Subtract, Some(s), Some(b)) => Some(b.subtract(&s)),
            (SelectionMode::Intersect, Some(s), Some(b)) => Some(b.intersect(&s)),
            (SelectionMode::Subtract | SelectionMode::Intersect, Some(_), None) => None,
        }
    }
}

/// Texto que se está escribiendo (todavía no aplicado).
#[derive(Clone, Debug, PartialEq)]
pub struct TextDraft {
    /// Esquina superior izquierda, en píxeles del artboard activo.
    pub origin: Point,
    pub text: String,
}

/// La herramienta que muestra cada grupo de la barra: la última elegida en él.
///
/// Se guarda con las opciones como una lista de nombres de variante (`"Lasso"`), uno por grupo en
/// el orden de `GROUPS`. Si falta un nombre, no se conoce o es de otro grupo, ese grupo vuelve a su
/// primera herramienta.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GroupChoice([ToolKind; ToolKind::GROUPS.len()]);

impl Default for GroupChoice {
    fn default() -> Self {
        Self(ToolKind::GROUPS.map(|g| g[0]))
    }
}

impl GroupChoice {
    /// La herramienta recordada del grupo `group`.
    pub fn get(&self, group: usize) -> ToolKind {
        self.0[group]
    }

    /// Recuerda `kind` como la elegida de su grupo.
    pub fn remember(&mut self, kind: ToolKind) {
        self.0[kind.group()] = kind;
    }
}

impl serde::Serialize for GroupChoice {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_seq(self.0.iter().map(|k| format!("{k:?}")))
    }
}

impl<'de> serde::Deserialize<'de> for GroupChoice {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let names = <Vec<String> as serde::Deserialize>::deserialize(d)?;
        let mut choice = GroupChoice::default();
        for (slot, (group, name)) in choice.0.iter_mut().zip(ToolKind::GROUPS.iter().zip(&names)) {
            if let Some(&kind) = group.iter().find(|k| format!("{k:?}") == *name) {
                *slot = kind;
            }
        }
        Ok(choice)
    }
}

/// Tamaño máximo del pincel (slider de Options y Ctrl+rueda).
pub const MAX_BRUSH_SIZE: u32 = 32;

/// Opciones compartidas por las herramientas. Se guardan al salir; lo que falte en un ajuste
/// guardado (por ejemplo `brush_shape`, de antes de que existiera) toma el valor por defecto.
#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct ToolOptions {
    /// Un ajuste guardado fuera de `1..=MAX_BRUSH_SIZE` se lleva al rango al leerlo.
    #[serde(deserialize_with = "clamped_brush_size")]
    pub brush_size: u32,
    pub brush_shape: BrushShape,
    /// El pincel personalizado pinta con los colores del dibujo del que salió (si no, con el
    /// color primario), como el de Aseprite.
    pub brush_colors: bool,
    pub pixel_perfect: bool,
    pub filled: bool,
    /// Radio de las esquinas del rectángulo (0 = rectas).
    pub corner_radius: u32,
    pub contiguous: bool,
    /// Shift presionado (lo actualiza la UI cada frame). No se guarda.
    #[serde(skip)]
    pub constrain: bool,
    /// No se guarda: cada sesión empieza en Replace.
    #[serde(skip)]
    pub selection_mode: SelectionMode,
    /// Varita: diferencia máxima por canal.
    pub tolerance: u8,
    /// Varita: tomar los colores de todas las capas visibles.
    pub sample_all: bool,
    pub spray_width: u32,
    pub spray_speed: u32,
    pub gradient_shape: GradientShape,
    pub dither: Dither,
    /// Cómo se rota la selección (herramientas de selección y Move).
    pub rotation: cs_raster::RotationAlgorithm,
    /// La variante que muestra cada grupo de la barra.
    pub group_choice: GroupChoice,
    /// Selección espejo: con simetría, lo seleccionado se refleja en los ejes, y al moverlo la
    /// parte reflejada se mueve como en un espejo.
    pub mirror_selection: bool,
    /// Cómo pintan las herramientas de color (ver `ToolKind::uses_ink`).
    pub ink: Ink,
}

/// Cómo se pinta encima de lo que hay, como los inks de Aseprite.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Ink {
    /// El color reemplaza lo que había.
    #[default]
    Simple,
    /// Solo donde ya hay algo pintado, conservando su transparencia.
    LockAlpha,
    /// Cada píxel pasa al color siguiente de la rampa (el rango elegido de la paleta o, si no
    /// hay, la paleta entera); con el botón derecho, al anterior. Lo que no está en la rampa
    /// no cambia.
    Shading,
}

impl Ink {
    pub const ALL: [Ink; 3] = [Ink::Simple, Ink::LockAlpha, Ink::Shading];

    pub fn label(self) -> &'static str {
        match self {
            Ink::Simple => "Simple",
            Ink::LockAlpha => "Lock Alpha",
            Ink::Shading => "Shading",
        }
    }
}

fn clamped_brush_size<'de, D: serde::Deserializer<'de>>(d: D) -> Result<u32, D::Error> {
    let size = <u32 as serde::Deserialize>::deserialize(d)?;
    Ok(size.clamp(1, MAX_BRUSH_SIZE))
}

impl Default for ToolOptions {
    fn default() -> Self {
        Self {
            brush_size: 1,
            brush_shape: BrushShape::Round,
            brush_colors: true,
            pixel_perfect: true,
            filled: false,
            corner_radius: 0,
            contiguous: true,
            constrain: false,
            selection_mode: SelectionMode::Replace,
            tolerance: 0,
            sample_all: false,
            spray_width: 16,
            spray_speed: 32,
            gradient_shape: GradientShape::Linear,
            dither: Dither::None,
            rotation: cs_raster::RotationAlgorithm::RotSprite,
            group_choice: GroupChoice::default(),
            mirror_selection: false,
            ink: Ink::Simple,
        }
    }
}
