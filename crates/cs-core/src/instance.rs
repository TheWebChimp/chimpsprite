//! Instancias: un artboard colocado dentro de otro, con su propia línea de tiempo.

use serde::{Deserialize, Serialize};

use crate::{ArtboardId, Sprite};

/// Cómo avanza la línea de tiempo del símbolo.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Timing {
    /// Sincronizado con los frames del padre (como los "Graphic" de Animate).
    Graphic {
        first_frame: usize,
        play: GraphicPlay,
    },
    /// Corre por tiempo, con las duraciones del símbolo, en loop (como los "Movie clip").
    MovieClip { first_frame: usize },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GraphicPlay {
    Loop,
    /// Avanza y se queda en el último frame.
    Once,
    /// Siempre el mismo frame.
    Single,
}

/// Estado de una instancia desde un frame del padre.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstanceKey {
    pub frame: usize,
    /// `None` = la capa no muestra nada desde este frame.
    pub source: Option<ArtboardId>,
    /// Esquina superior izquierda del símbolo, en píxeles del padre.
    pub position: (i32, i32),
    pub flip_h: bool,
    pub flip_v: bool,
    pub opacity: u8,
    pub timing: Timing,
}

impl InstanceKey {
    /// Key en loop desde el primer frame, sin flip y opaca.
    pub fn new(frame: usize, source: ArtboardId, position: (i32, i32)) -> Self {
        Self {
            frame,
            source: Some(source),
            position,
            flip_h: false,
            flip_v: false,
            opacity: 255,
            timing: Timing::Graphic {
                first_frame: 0,
                play: GraphicPlay::Loop,
            },
        }
    }
}

/// Keys de una capa de instancia, ordenadas por frame; cada una vale hasta la siguiente.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstanceTrack {
    pub keys: Vec<InstanceKey>,
}

impl InstanceTrack {
    /// Key vigente en `frame` (la última que empieza en o antes de él).
    pub fn key_at(&self, frame: usize) -> Option<&InstanceKey> {
        self.keys.iter().rev().find(|k| k.frame <= frame)
    }

    /// Pone (o reemplaza) la key de su frame, manteniendo el orden.
    pub fn set_key(&mut self, key: InstanceKey) {
        match self.keys.binary_search_by_key(&key.frame, |k| k.frame) {
            Ok(i) => self.keys[i] = key,
            Err(i) => self.keys.insert(i, key),
        }
    }

    /// Quita la key que empieza en `frame`. `false` si no había.
    pub fn remove_key(&mut self, frame: usize) -> bool {
        let before = self.keys.len();
        self.keys.retain(|k| k.frame != frame);
        self.keys.len() != before
    }
}

/// Frame del símbolo `source` que se ve en el frame `frame` del padre, según `key`.
pub fn source_frame(key: &InstanceKey, parent: &Sprite, frame: usize, source: &Sprite) -> usize {
    let n = source.frames.len();
    if n == 0 {
        return 0;
    }
    let elapsed_frames = frame.saturating_sub(key.frame);
    match key.timing {
        Timing::Graphic { first_frame, play } => {
            let f0 = first_frame.min(n - 1);
            match play {
                GraphicPlay::Loop => (f0 + elapsed_frames) % n,
                GraphicPlay::Once => (f0 + elapsed_frames).min(n - 1),
                GraphicPlay::Single => f0,
            }
        }
        Timing::MovieClip { first_frame } => {
            let f0 = first_frame.min(n - 1);
            let total: u64 = source.frames.iter().map(|f| f.duration_ms as u64).sum();
            if total == 0 {
                return f0;
            }
            let end = frame.min(parent.frames.len());
            let start = key.frame.min(end);
            let elapsed: u64 = parent.frames[start..end]
                .iter()
                .map(|f| f.duration_ms as u64)
                .sum();
            let mut t = elapsed % total;
            let mut f = f0;
            // Termina: `total > 0`, así que alguna duración es mayor que lo que queda.
            loop {
                let duration = source.frames[f].duration_ms as u64;
                if t < duration {
                    return f;
                }
                t -= duration;
                f = (f + 1) % n;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ColorMode, Frame, Sprite};

    fn sprite(durations: &[u16]) -> Sprite {
        let mut s = Sprite::new(4, 4, ColorMode::Rgba);
        s.frames = durations
            .iter()
            .map(|&d| Frame { duration_ms: d })
            .collect();
        s
    }

    fn key(frame: usize, timing: Timing) -> InstanceKey {
        InstanceKey {
            timing,
            ..InstanceKey::new(frame, ArtboardId(2), (0, 0))
        }
    }

    fn graphic(first_frame: usize, play: GraphicPlay) -> Timing {
        Timing::Graphic { first_frame, play }
    }

    #[test]
    fn graphic_modes_follow_parent_frames() {
        let parent = sprite(&[100; 10]);
        let source = sprite(&[100; 3]);
        let at = |t: Timing, f: usize| source_frame(&key(2, t), &parent, f, &source);
        // Loop desde el frame 1 del símbolo: 1, 2, 0, 1…
        let looped: Vec<_> = (2..7)
            .map(|f| at(graphic(1, GraphicPlay::Loop), f))
            .collect();
        assert_eq!(looped, vec![1, 2, 0, 1, 2]);
        // Una vez: se queda en el último.
        let once: Vec<_> = (2..7)
            .map(|f| at(graphic(0, GraphicPlay::Once), f))
            .collect();
        assert_eq!(once, vec![0, 1, 2, 2, 2]);
        // Fijo.
        assert_eq!(at(graphic(2, GraphicPlay::Single), 9), 2);
        // first_frame fuera de rango se recorta al último.
        assert_eq!(at(graphic(7, GraphicPlay::Single), 2), 2);
    }

    #[test]
    fn movie_clip_runs_on_time() {
        // Padre a 50 ms por frame; símbolo con frames de 100, 50 y 50 ms (200 ms en total).
        let parent = sprite(&[50; 12]);
        let source = sprite(&[100, 50, 50]);
        let k = key(1, Timing::MovieClip { first_frame: 0 });
        let frames: Vec<_> = (1..7)
            .map(|f| source_frame(&k, &parent, f, &source))
            .collect();
        // t = 0, 50, 100, 150, 200 (vuelve), 250.
        assert_eq!(frames, vec![0, 0, 1, 2, 0, 0]);
        // Empezando en el frame 2 del símbolo.
        let k = key(0, Timing::MovieClip { first_frame: 2 });
        assert_eq!(source_frame(&k, &parent, 1, &source), 0);
    }

    #[test]
    fn movie_clip_with_zero_total_duration_stays_on_first_frame() {
        let parent = sprite(&[100; 4]);
        let source = sprite(&[0, 0]);
        let k = key(0, Timing::MovieClip { first_frame: 1 });
        assert_eq!(source_frame(&k, &parent, 3, &source), 1);
    }

    #[test]
    fn track_keys_apply_until_the_next_one() {
        let mut t = InstanceTrack::default();
        t.set_key(InstanceKey::new(2, ArtboardId(2), (1, 1)));
        t.set_key(InstanceKey::new(0, ArtboardId(2), (0, 0)));
        t.set_key(InstanceKey::new(2, ArtboardId(3), (5, 5)));
        assert_eq!(
            t.keys.iter().map(|k| k.frame).collect::<Vec<_>>(),
            vec![0, 2]
        );
        assert_eq!(t.key_at(1).unwrap().position, (0, 0));
        assert_eq!(t.key_at(4).unwrap().source, Some(ArtboardId(3)));
        assert!(t.remove_key(2));
        assert!(!t.remove_key(2));
        assert_eq!(t.key_at(4).unwrap().position, (0, 0));
        let mut late = InstanceTrack::default();
        late.set_key(InstanceKey::new(3, ArtboardId(2), (0, 0)));
        assert!(late.key_at(1).is_none(), "nothing before the first key");
    }

    #[test]
    fn instance_layers_serialize_and_old_layers_still_load() {
        let mut layer = crate::Layer::new_image(crate::LayerId(1), "a");
        layer.kind = crate::LayerKind::Instance;
        layer.instance = Some(InstanceTrack {
            keys: vec![InstanceKey::new(0, ArtboardId(2), (3, 4))],
        });
        let json = serde_json::to_string(&layer).unwrap();
        assert_eq!(serde_json::from_str::<crate::Layer>(&json).unwrap(), layer);
        let old = r#"{"id":1,"name":"x","kind":"Image","parent":null,"visible":true,
            "locked":false,"opacity":255,"blend":"Normal"}"#;
        assert_eq!(
            serde_json::from_str::<crate::Layer>(old).unwrap().instance,
            None
        );
    }
}
