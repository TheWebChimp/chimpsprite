//! La entrada sobre la perspectiva del workspace del modelo (el cuadrante Persp): los gestos
//! mueven la cámara o eligen objetos.
//!
//! - **Botón derecho o Alt + izquierdo** arrastrando: orbita (`yaw` y `pitch`).
//! - **Botón central o Shift + izquierdo** arrastrando: mueve el `target` en el plano de la vista
//!   (paneo).
//! - **Rueda** (con o sin Ctrl): acerca o aleja la cámara (`distance`) hacia el punto bajo el
//!   puntero, sin tocar el zoom del lienzo.
//! - **Gizmo de navegación** (arriba a la derecha): arrastrarlo orbita; un clic en una bolita
//!   pone la cámara mirando desde ese lado (al centro del modelo).
//! - **Home:** encuadra todo el modelo (el objetivo a su centro, la distancia para que se vea
//!   entero).
//! - **Clic izquierdo:** elige el objeto que se ve bajo el puntero (o ninguno). En el render del
//!   modo Texture elige la cara (Shift + clic sin arrastrar la agrega o la quita).
//!
//! Cada arrastre es un solo paso de undo.

use cs_core::{Camera3D, FIXED, Model3D, Vec3i};

use super::*;
use crate::editor::PerspMap;

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
    /// Pixeles del artboard por punto de pantalla (el render de la perspectiva va a escala
    /// entera).
    factor: Vec2,
    /// En el render del modo Texture, Shift + clic: el pixel del render donde se apretó. Si se
    /// suelta sin pasar el umbral de arrastre, agrega o quita la cara de ahí (y la cámara no se
    /// movió); pasado el umbral, mueve el objetivo.
    toggle: Option<(i32, i32)>,
    /// Se apretó el gizmo de navegación y todavía no se arrastró: al soltar así, la cámara mira
    /// desde el lado de la bolita (si se apretó una); pasado el umbral, orbita.
    nav: Option<Option<super::nav_gizmo::NavBall>>,
}

impl ModelDrag {
    /// Apretar el gizmo de navegación en `from` (sobre la bolita `ball`, si hay) con la cámara
    /// `camera`.
    pub(super) fn nav(
        from: Pos2,
        camera: Camera3D,
        ball: Option<super::nav_gizmo::NavBall>,
    ) -> ModelDrag {
        ModelDrag {
            orbit: true,
            from,
            camera,
            factor: Vec2::ONE,
            toggle: None,
            nav: Some(ball),
        }
    }
}

/// Cuánto sin rueda corta la ráfaga de pasos (segundos).
const WHEEL_PAUSE: f64 = 0.6;

/// El grupo de undo de la rueda sobre la cámara del modelo.
fn wheel_group() -> egui::Id {
    egui::Id::new("model_wheel")
}

/// El grupo de undo de los arrastres sobre el modelo.
fn drag_group() -> egui::Id {
    egui::Id::new("model_drag")
}

impl CanvasView {
    /// `steps` pasos de rueda sobre el modelo: hacia arriba acerca la cámara. Con `at` (cómo se
    /// ve la perspectiva y el puntero), el objetivo se corre hacia el punto bajo el puntero en su
    /// plano (como "zoom to mouse position" de Blender): ese punto se queda donde está en pantalla.
    ///
    /// Los pasos seguidos (con menos de [`WHEEL_PAUSE`] entre uno y otro, `now` en segundos) son un
    /// solo paso de undo; otro gesto o una pausa empieza uno nuevo.
    pub(super) fn model_wheel(
        &mut self,
        editor: &mut Editor,
        steps: i32,
        at: Option<(PerspMap, Pos2)>,
        now: f64,
    ) {
        let Some(model) = editor.edited_model().cloned() else {
            return;
        };
        let mut camera = model.camera;
        let factor = DISTANCE_STEP.powi(-steps);
        let distance = ((camera.distance as f32 * factor).round() as i32).max(FIXED);
        if let Some((map, p)) = at {
            let k = 1.0 - distance as f32 / camera.distance.max(1) as f32;
            let to = focal_point(&map, p);
            let t = camera.target;
            let toward = |from: i32, to: f32| from + ((to - from as f32) * k).round() as i32;
            camera.target = Vec3i {
                x: toward(t.x, to[0]),
                y: toward(t.y, to[1]),
                z: toward(t.z, to[2]),
            };
        }
        camera.distance = distance;
        if self.model_wheel_at.is_none_or(|t| now - t > WHEEL_PAUSE) {
            editor.end_group_of(wheel_group().value());
        }
        self.model_wheel_at = Some(now);
        editor.set_model_grouped(Model3D { camera, ..model }, wheel_group());
    }

    /// Un botón apretado sobre la perspectiva, con el punto ya en pixeles del artboard (`local`)
    /// y el `factor` de pantalla a pixeles del artboard: empieza a orbitar (derecho o Alt +
    /// izquierdo), a mover el objetivo (Shift + izquierdo) o elige el objeto bajo el puntero
    /// (izquierdo).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn model_press_at(
        &mut self,
        editor: &mut Editor,
        p: Pos2,
        local: (i32, i32),
        secondary: bool,
        modifiers: egui::Modifiers,
        factor: Vec2,
    ) {
        editor.end_group();
        let Some(camera) = editor.edited_model().map(|m| m.camera) else {
            return;
        };
        if secondary || modifiers.alt || modifiers.shift {
            self.model_drag = Some(ModelDrag {
                orbit: secondary || modifiers.alt,
                from: p,
                camera,
                factor,
                toggle: None,
                nav: None,
            });
        } else {
            editor.pick_model_object(local);
        }
    }

    /// El botón central apretado sobre la perspectiva (con el `factor` de pantalla a pixeles del
    /// artboard): empieza a mover el objetivo (paneo), como Shift + izquierdo.
    pub(super) fn model_pan_press(&mut self, editor: &mut Editor, p: Pos2, factor: Vec2) {
        editor.end_group();
        let Some(camera) = editor.edited_model().map(|m| m.camera) else {
            return;
        };
        self.model_drag = Some(ModelDrag {
            orbit: false,
            from: p,
            camera,
            factor,
            toggle: None,
            nav: None,
        });
    }

    /// Como [`Self::model_press_at`], en el render del modo Texture: el clic izquierdo elige la
    /// cara bajo el puntero; Shift + izquierdo la agrega o la quita si no se arrastra (si se
    /// arrastra, mueve el objetivo).
    pub(super) fn texture_press_at(
        &mut self,
        editor: &mut Editor,
        p: Pos2,
        local: (i32, i32),
        secondary: bool,
        modifiers: egui::Modifiers,
        factor: Vec2,
    ) {
        editor.end_group();
        let Some(camera) = editor.edited_model().map(|m| m.camera) else {
            return;
        };
        if secondary || modifiers.alt || modifiers.shift {
            let orbit = secondary || modifiers.alt;
            self.model_drag = Some(ModelDrag {
                orbit,
                from: p,
                camera,
                factor,
                toggle: (!orbit).then_some(local),
                nav: None,
            });
        } else {
            editor.pick_model_face(local, false);
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
        let Some(mut drag) = self.model_drag else {
            return;
        };
        if !down || editor.edited_model().is_none() {
            editor.end_group_of(drag_group().value());
            self.model_drag = None;
            // Shift + clic sin arrastre en el render del modo Texture.
            if let Some(local) = drag.toggle
                && editor.edited_model().is_some()
            {
                editor.pick_model_face(local, true);
            }
            // Un clic en una bolita del gizmo de navegación: un paso de undo, como los presets (y
            // como ellos, mirando al centro del modelo).
            if let Some(Some(ball)) = drag.nav {
                let mut camera = ball.camera(drag.camera);
                if let Some(target) = editor.edited_model().and_then(crate::editor::model_center) {
                    camera.target = target;
                }
                editor.set_camera(camera);
            }
            return;
        }
        let (Some(p), Some(mut model)) = (pointer, editor.edited_model().cloned()) else {
            return;
        };
        let d = p - drag.from;
        if drag.toggle.is_some() || drag.nav.is_some() {
            if d.length() < crate::editor::DRAG_THRESHOLD {
                return;
            }
            // Ya es un arrastre: mueve la cámara y al soltar no elige nada.
            drag.toggle = None;
            drag.nav = None;
            self.model_drag = Some(drag);
        }
        model.camera = if drag.orbit {
            orbit(drag.camera, d)
        } else {
            // Pixeles del artboard: lo que se arrastró en pantalla, sin la escala del render.
            let pixels = d * drag.factor;
            pan(
                drag.camera,
                pixels,
                super::model_edit::model_sprite(editor).height,
            )
        };
        editor.set_model_grouped(model, drag_group());
    }
}

/// Los eventos de rueda de este frame (unidad y vertical), con o sin modificadores.
pub(super) fn wheel_events(ui: &egui::Ui) -> Vec<(egui::MouseWheelUnit, f32)> {
    ui.input(|i| {
        i.raw
            .events
            .iter()
            .filter_map(|e| match e {
                egui::Event::MouseWheel { unit, delta, .. } => Some((*unit, delta.y)),
                _ => None,
            })
            .collect()
    })
}

/// El punto del plano del objetivo (perpendicular a la mirada) que se ve en `p`, en 1/16 de
/// unidad.
fn focal_point(map: &PerspMap, p: Pos2) -> [f32; 3] {
    let (o, d) = map.ray((p.x, p.y));
    let fwd = map.projector().axes()[2];
    let t = map.camera.target;
    let t = [t.x as f32, t.y as f32, t.z as f32];
    let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let along = dot(d, fwd);
    if along.abs() < f32::EPSILON {
        return t;
    }
    let s = dot([t[0] - o[0], t[1] - o[1], t[2] - o[2]], fwd) / along;
    std::array::from_fn(|i| o[i] + d[i] * s)
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
