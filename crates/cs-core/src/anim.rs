use serde::{Deserialize, Serialize};

use crate::Rgba;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnimDirection {
    #[default]
    Forward,
    Reverse,
    PingPong,
    /// Ping-pong que empieza hacia atrás.
    PingPongReverse,
}

/// Rango con nombre de frames (inclusivo), como los tags de Aseprite.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tag {
    pub name: String,
    pub from: usize,
    pub to: usize,
    pub direction: AnimDirection,
    pub color: Rgba,
    /// Veces que se repite (0 = sin especificar, como en Aseprite).
    #[serde(default)]
    pub repeat: u16,
}

impl Tag {
    pub fn contains(&self, frame: usize) -> bool {
        (self.from..=self.to).contains(&frame)
    }
}

/// Estado de reproducción: frame actual y sentido (para ping-pong).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Playback {
    pub frame: usize,
    pub forward: bool,
    /// Vueltas que faltan (las del `repeat` del tag); `None`, sin fin.
    pub remaining: Option<u16>,
}

impl Playback {
    pub fn new(frame: usize, direction: AnimDirection) -> Self {
        Self {
            frame,
            forward: !matches!(
                direction,
                AnimDirection::Reverse | AnimDirection::PingPongReverse
            ),
            remaining: None,
        }
    }

    /// Que se repita `repeat` veces (0: sin fin, como en Aseprite).
    pub fn repeating(mut self, repeat: u16) -> Self {
        self.remaining = (repeat > 0).then_some(repeat);
        self
    }

    /// Avanza un frame dentro de `range` (inclusivo) según `direction`. Devuelve `false` si ya
    /// dio las vueltas de `remaining`: entonces se queda en el último frame de la última vuelta.
    pub fn step(&mut self, range: (usize, usize), direction: AnimDirection) -> bool {
        let (from, to) = (range.0.min(range.1), range.0.max(range.1));
        if !(from..=to).contains(&self.frame) {
            self.frame = if direction == AnimDirection::Reverse {
                to
            } else {
                from
            };
            return true;
        }
        // Termina una vuelta: Forward al pasar del último al primero, Reverse al revés y
        // ping-pong al volver al frame del que salió.
        let lap = match direction {
            AnimDirection::Forward => self.frame == to,
            AnimDirection::Reverse => self.frame == from,
            AnimDirection::PingPong => !self.forward && self.frame == from,
            AnimDirection::PingPongReverse => self.forward && self.frame == to,
        };
        if lap && let Some(left) = self.remaining {
            if left <= 1 {
                self.remaining = Some(0);
                return false;
            }
            self.remaining = Some(left - 1);
        }
        if from == to {
            return true;
        }
        match direction {
            AnimDirection::Forward => {
                self.frame = if self.frame == to {
                    from
                } else {
                    self.frame + 1
                };
            }
            AnimDirection::Reverse => {
                self.frame = if self.frame == from {
                    to
                } else {
                    self.frame - 1
                };
            }
            AnimDirection::PingPong | AnimDirection::PingPongReverse => {
                if self.forward && self.frame == to {
                    self.forward = false;
                } else if !self.forward && self.frame == from {
                    self.forward = true;
                }
                self.frame = if self.forward {
                    self.frame + 1
                } else {
                    self.frame - 1
                };
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tag_with_repeat_plays_that_many_laps_and_stops_on_its_last_frame() {
        let mut p = Playback::new(0, AnimDirection::Forward).repeating(2);
        let frames: Vec<_> = (0..7)
            .map_while(|_| p.step((0, 2), AnimDirection::Forward).then_some(p.frame))
            .collect();
        assert_eq!(frames, vec![1, 2, 0, 1, 2], "two laps");
        assert_eq!(p.frame, 2, "stays on the last frame");
        // Ping-pong: una vuelta es ida y vuelta.
        let mut p = Playback::new(0, AnimDirection::PingPong).repeating(1);
        let frames: Vec<_> = (0..9)
            .map_while(|_| p.step((0, 2), AnimDirection::PingPong).then_some(p.frame))
            .collect();
        assert_eq!(frames, vec![1, 2, 1, 0]);
        // Sin repeat, sin fin.
        let mut p = Playback::new(0, AnimDirection::Forward);
        assert!((0..50).all(|_| p.step((0, 2), AnimDirection::Forward)));
    }

    fn run(direction: AnimDirection, start: usize, range: (usize, usize), n: usize) -> Vec<usize> {
        let mut p = Playback::new(start, direction);
        (0..n)
            .map(|_| {
                p.step(range, direction);
                p.frame
            })
            .collect()
    }

    #[test]
    fn forward_and_reverse_wrap() {
        assert_eq!(run(AnimDirection::Forward, 1, (1, 3), 4), vec![2, 3, 1, 2]);
        assert_eq!(run(AnimDirection::Reverse, 3, (1, 3), 4), vec![2, 1, 3, 2]);
    }

    #[test]
    fn ping_pong_bounces() {
        assert_eq!(
            run(AnimDirection::PingPong, 0, (0, 2), 6),
            vec![1, 2, 1, 0, 1, 2]
        );
    }

    #[test]
    fn ping_pong_reverse_starts_backwards() {
        assert_eq!(
            run(AnimDirection::PingPongReverse, 2, (0, 2), 4),
            vec![1, 0, 1, 2]
        );
    }

    #[test]
    fn single_frame_and_outside_range() {
        assert_eq!(run(AnimDirection::PingPong, 4, (4, 4), 3), vec![4, 4, 4]);
        assert_eq!(run(AnimDirection::Forward, 9, (2, 5), 2), vec![2, 3]);
        assert_eq!(run(AnimDirection::Reverse, 0, (2, 5), 1), vec![5]);
    }
}
