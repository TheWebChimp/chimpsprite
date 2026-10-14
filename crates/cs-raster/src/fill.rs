use crate::Point;

/// Relleno 4-conexo desde `start` sobre los píxeles donde `matches` es verdadero.
pub fn flood_fill(
    width: u32,
    height: u32,
    start: Point,
    matches: impl Fn(i32, i32) -> bool,
) -> Vec<Point> {
    let (w, h) = (width as i32, height as i32);
    let inside = |(x, y): Point| x >= 0 && y >= 0 && x < w && y < h;
    if !inside(start) || !matches(start.0, start.1) {
        return Vec::new();
    }
    let index = |(x, y): Point| (y * w + x) as usize;
    let mut visited = vec![false; (w * h) as usize];
    visited[index(start)] = true;
    let mut stack = vec![start];
    let mut out = Vec::new();
    while let Some((x, y)) = stack.pop() {
        out.push((x, y));
        for n in [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)] {
            if inside(n) && !visited[index(n)] && matches(n.0, n.1) {
                visited[index(n)] = true;
                stack.push(n);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fills_l_region_without_crossing_diagonals() {
        // 1 = región, 0 = muro. (2,2) solo toca a la región en diagonal.
        let grid = ["110", "100", "001"];
        let at = |x: i32, y: i32| grid[y as usize].as_bytes()[x as usize] == b'1';
        let mut got = flood_fill(3, 3, (0, 0), at);
        got.sort();
        assert_eq!(got, vec![(0, 0), (0, 1), (1, 0)]);
    }

    #[test]
    fn start_outside_or_not_matching_is_empty() {
        assert!(flood_fill(2, 2, (5, 5), |_, _| true).is_empty());
        assert!(flood_fill(2, 2, (0, 0), |_, _| false).is_empty());
    }
}
