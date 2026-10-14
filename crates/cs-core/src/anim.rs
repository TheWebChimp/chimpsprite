use serde::{Deserialize, Serialize};

use crate::Rgba;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnimDirection {
    #[default]
    Forward,
    Reverse,
    PingPong,
}

/// Rango con nombre de frames (inclusivo), como los tags de Aseprite.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tag {
    pub name: String,
    pub from: usize,
    pub to: usize,
    pub direction: AnimDirection,
    pub color: Rgba,
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
}

impl Playback {
    pub fn new(frame: usize, direction: AnimDirection) -> Self {
        Self {
            frame,
            forward: direction != AnimDirection::Reverse,
        }
    }

    /// Avanza un frame dentro de `range` (inclusivo) según `direction`.
    pub fn step(&mut self, range: (usize, usize), direction: AnimDirection) {
        let (from, to) = (range.0.min(range.1), range.0.max(range.1));
        if !(from..=to).contains(&self.frame) {
            self.frame = if direction == AnimDirection::Reverse {
                to
            } else {
                from
            };
            return;
        }
        if from == to {
            return;
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
            AnimDirection::PingPong => {
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn single_frame_and_outside_range() {
        assert_eq!(run(AnimDirection::PingPong, 4, (4, 4), 3), vec![4, 4, 4]);
        assert_eq!(run(AnimDirection::Forward, 9, (2, 5), 2), vec![2, 3]);
        assert_eq!(run(AnimDirection::Reverse, 0, (2, 5), 1), vec![5]);
    }
}
