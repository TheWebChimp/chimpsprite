use crate::{Point, normalize};

pub fn rect_filled(a: Point, b: Point) -> Vec<Point> {
    let ((x0, y0), (x1, y1)) = normalize(a, b);
    (y0..=y1)
        .flat_map(|y| (x0..=x1).map(move |x| (x, y)))
        .collect()
}

pub fn rect_outline(a: Point, b: Point) -> Vec<Point> {
    let ((x0, y0), (x1, y1)) = normalize(a, b);
    if x0 == x1 || y0 == y1 {
        return rect_filled(a, b);
    }
    let mut out: Vec<Point> = (x0..=x1).map(|x| (x, y0)).collect();
    for y in y0 + 1..y1 {
        out.push((x0, y));
        out.push((x1, y));
    }
    out.extend((x0..=x1).map(|x| (x, y1)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_sizes() {
        assert_eq!(rect_outline((2, 2), (2, 2)), vec![(2, 2)]);
        assert_eq!(rect_outline((0, 0), (2, 2)).len(), 8);
        assert_eq!(rect_filled((0, 0), (2, 2)).len(), 9);
        assert_eq!(rect_outline((0, 0), (3, 0)).len(), 4);
    }

    #[test]
    fn inverted_corners_are_normalized() {
        assert_eq!(rect_outline((2, 3), (0, 0)), rect_outline((0, 0), (2, 3)));
        assert_eq!(rect_filled((2, 3), (0, 0)), rect_filled((0, 0), (2, 3)));
    }
}
