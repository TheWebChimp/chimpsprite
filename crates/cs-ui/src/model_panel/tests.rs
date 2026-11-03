use super::*;

#[test]
fn the_three_axes_share_a_row_when_they_fit_and_go_to_two_rows_when_not() {
    // El panel de siempre: los tres en una fila, del mismo ancho, con su aire entre ellos.
    let (per_row, w) = axis_grid(WIDTH - pixel_ui::SCROLL_BAR - AXIS_GAP);
    assert_eq!(per_row, 3);
    assert_eq!(
        w,
        ((WIDTH - pixel_ui::SCROLL_BAR - 3.0 * AXIS_GAP) / 3.0).floor()
    );
    assert!(w >= AXIS_MIN_W);
    // Angosto: dos por fila (X e Y; Z debajo), más anchos.
    let narrow = 2.0 * AXIS_MIN_W + 2.0 * AXIS_GAP;
    let (per_row, w) = axis_grid(narrow);
    assert_eq!(per_row, 2);
    assert!(w >= AXIS_MIN_W && 2.0 * w + AXIS_GAP <= narrow);
    // Justo lo de tres.
    assert_eq!(axis_grid(3.0 * AXIS_MIN_W + 2.0 * AXIS_GAP).0, 3);
    assert_eq!(axis_grid(3.0 * AXIS_MIN_W + 2.0 * AXIS_GAP - 1.0).0, 2);
}
