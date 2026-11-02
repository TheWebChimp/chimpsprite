//! La entrada del lienzo sobre el artboard 3D activo: con el puntero en su área de pixeles, los
//! gestos mueven la cámara o eligen objetos en vez de llegar a las herramientas.
//!
//! - **Botón derecho o Alt + izquierdo** arrastrando: orbita (`yaw` y `pitch`).
//! - **Shift + izquierdo** arrastrando: mueve el `target` en el plano de la vista.
//! - **Ctrl + rueda:** acerca o aleja la cámara (`distance`), sin tocar el zoom del lienzo.
//! - **Clic izquierdo:** elige el objeto que se ve bajo el puntero (o ninguno).
//!
//! Cada arrastre es un solo paso de undo.

use cs_core::{Camera3D, FIXED, Vec3i};

use super::*;

/// Décimas de grado por punto de pantalla al orbitar.
const ORBIT_SPEED: f32 = 5.0;
/// El `pitch` de la cámara no pasa de ±89°.
const PITCH_LIMIT: i32 = 890;
/// Cuánto cambia la distancia por paso de rueda.
const DISTANCE_STEP: f32 = 1.1;

/// Un arrastre sobre el artboard 3D: orbitar o mover el objetivo, desde dónde y con la cámara
/// del principio (el cambio se calcula siempre desde ahí).
#[derive(Clone, Copy, Debug)]
pub(super) struct ModelDrag {
    orbit: bool,
    from: Pos2,
    camera: Camera3D,
    /// Pixeles del artboard entero por pixel del render (1 fuera del modo Edit; en la perspectiva
    /// del modo Edit, el render es más chico que el artboard).
    factor: f32,
}

/// El grupo de undo de los arrastres sobre el modelo.
fn drag_group() -> egui::Id {
    egui::Id::new("model_drag")
}

impl CanvasView {
    /// Si `p` cae en el área de pixeles del artboard activo, cuando es un modelo 3D y no lo tapa
    /// otro artboard.
    pub(super) fn over_model(&self, rect: Rect, editor: &Editor, p: Pos2, ppp: f32) -> bool {
        if !editor.is_model() {
            return false;
        }
        let (min, max) = editor.artboard().rect();
        let area = Rect::from_min_max(self.to_screen(rect, min), self.to_screen(rect, max));
        area.contains(p) && !self.under_front_artboard(rect, editor, p, ppp)
    }

    /// `steps` pasos de rueda sobre el modelo: hacia arriba acerca la cámara.
    pub(super) fn model_wheel(editor: &mut Editor, steps: i32) {
        let Some(mut camera) = editor.model().map(|m| m.camera) else {
            return;
        };
        let factor = DISTANCE_STEP.powi(-steps);
        camera.distance = ((camera.distance as f32 * factor).round() as i32).max(FIXED);
        editor.set_camera(camera);
    }

    /// Un botón apretado sobre el modelo: empieza a orbitar (derecho o Alt + izquierdo), a mover
    /// el objetivo (Shift + izquierdo) o elige el objeto bajo el puntero (izquierdo).
    pub(super) fn model_press(
        &mut self,
        rect: Rect,
        editor: &mut Editor,
        p: Pos2,
        secondary: bool,
        modifiers: egui::Modifiers,
    ) {
        let local = editor.to_local(self.to_world(rect, p));
        self.model_press_at(editor, p, local, secondary, modifiers, 1.0);
    }

    /// Lo mismo con el punto ya en pixeles del artboard (`local`) y el `factor` de pantalla a
    /// pixeles del artboard.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn model_press_at(
        &mut self,
        editor: &mut Editor,
        p: Pos2,
        local: (i32, i32),
        secondary: bool,
        modifiers: egui::Modifiers,
        factor: f32,
    ) {
        editor.end_group();
        let Some(camera) = editor.model().map(|m| m.camera) else {
            return;
        };
        if secondary || modifiers.alt || modifiers.shift {
            self.model_drag = Some(ModelDrag {
                orbit: secondary || modifiers.alt,
                from: p,
                camera,
                factor,
            });
        } else {
            editor.pick_model_object(local);
        }
    }

    /// El arrastre en curso sobre el modelo: la cámara sigue al puntero; al soltar, termina el
    /// paso de undo.
    pub(super) fn model_drag_input(
        &mut self,
        editor: &mut Editor,
        pointer: Option<Pos2>,
        down: bool,
    ) {
        let Some(drag) = self.model_drag else {
            return;
        };
        if !down || !editor.is_model() {
            editor.end_group_of(drag_group().value());
            self.model_drag = None;
            return;
        }
        let (Some(p), Some(mut model)) = (pointer, editor.model().cloned()) else {
            return;
        };
        let d = p - drag.from;
        model.camera = if drag.orbit {
            orbit(drag.camera, d)
        } else {
            // Pixeles del artboard: lo que se arrastró en pantalla, sin el zoom.
            let pixels = d / self.scale() * drag.factor;
            pan(drag.camera, pixels, editor.sprite().height)
        };
        editor.set_model_grouped(model, drag_group());
    }
}

/// La cámara de `start` girada por un arrastre de `d` puntos (el `yaw`, siempre en ±180°).
fn orbit(start: Camera3D, d: Vec2) -> Camera3D {
    Camera3D {
        yaw: crate::editor::wrap_angle(start.yaw + (d.x * ORBIT_SPEED).round() as i32),
        pitch: (start.pitch + (d.y * ORBIT_SPEED).round() as i32).clamp(-PITCH_LIMIT, PITCH_LIMIT),
        ..start
    }
}

/// La cámara de `start` con el objetivo corrido en el plano de la vista para que lo de adentro
/// siga a un arrastre de `pixels` pixeles del artboard (de `height` de alto): a la distancia del
/// objetivo, la imagen mide `2·distance` de alto.
fn pan(start: Camera3D, pixels: Vec2, height: u32) -> Camera3D {
    let per_pixel = 2.0 * start.distance.max(1) as f32 / height.max(1) as f32;
    let (sy, cy) = (start.yaw as f32 / 10.0).to_radians().sin_cos();
    let (sp, cp) = (start.pitch as f32 / 10.0).to_radians().sin_cos();
    // La misma base que el rasterizador: `right` horizontal y `up = right × fwd`.
    let right = [cy, 0.0, -sy];
    let fwd = [-sy * cp, -sp, -cy * cp];
    let up = [
        right[1] * fwd[2] - right[2] * fwd[1],
        right[2] * fwd[0] - right[0] * fwd[2],
        right[0] * fwd[1] - right[1] * fwd[0],
    ];
    let shift = |i: usize| (-right[i] * pixels.x + up[i] * pixels.y) * per_pixel;
    let t = start.target;
    Camera3D {
        target: Vec3i {
            x: t.x + shift(0).round() as i32,
            y: t.y + shift(1).round() as i32,
            z: t.z + shift(2).round() as i32,
        },
        ..start
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::vec2;

    fn front() -> Camera3D {
        Camera3D {
            yaw: 0,
            pitch: 0,
            distance: 4 * FIXED,
            ..cs_core::Model3D::default().camera
        }
    }

    #[test]
    fn orbiting_limits_the_pitch() {
        let c = orbit(front(), vec2(2.0, 1000.0));
        assert_eq!((c.yaw, c.pitch), (10, PITCH_LIMIT));
        assert_eq!(orbit(front(), vec2(0.0, -1000.0)).pitch, -PITCH_LIMIT);
    }

    #[test]
    fn orbiting_wraps_the_yaw_around() {
        // 720° de vuelta: queda donde estaba.
        assert_eq!(orbit(front(), vec2(1440.0, 0.0)).yaw, 0);
        let near = Camera3D {
            yaw: 1790,
            ..front()
        };
        assert_eq!(orbit(near, vec2(4.0, 0.0)).yaw, -1790);
        assert_eq!(orbit(front(), vec2(-400.0, 0.0)).yaw, 1600);
    }

    #[test]
    fn panning_moves_the_target_against_the_drag() {
        // De frente (mirando hacia -Z), 32 pixeles de alto a distancia 4: 1/4 de unidad cada uno.
        let c = pan(front(), vec2(4.0, 0.0), 32);
        assert_eq!(c.target, Vec3i { x: -16, y: 0, z: 0 });
        let c = pan(front(), vec2(0.0, 4.0), 32);
        assert_eq!(c.target, Vec3i { x: 0, y: 16, z: 0 });
    }
}
